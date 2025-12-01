//! AI-powered enhancement for recorded workflow steps
//! Uses vision models to generate human-readable descriptions from screenshots

use anyhow::{anyhow, Result};
use genai::chat::{ChatMessage, ChatRequest, ContentPart};
use genai::Client;
use serde::Deserialize;

use crate::models::WorkflowStep;

/// Default model for AI enhancement (vision-capable)
const DEFAULT_ENHANCEMENT_MODEL: &str = "gemini-2.5-flash";

/// Maximum steps to process in a single API batch
const BATCH_SIZE: usize = 5;

/// AI enhancement service for recorded workflows
pub struct AIEnhancer {
    client: Client,
    model: String,
}

#[derive(Debug, Deserialize)]
struct StepDescription {
    step: usize,
    description: String,
}

impl AIEnhancer {
    pub fn new(model: Option<&str>) -> Self {
        Self {
            client: Client::default(),
            model: model.unwrap_or(DEFAULT_ENHANCEMENT_MODEL).to_string(),
        }
    }

    /// Enhance all steps with AI-generated descriptions
    /// Processes in batches to avoid token limits
    pub async fn enhance_steps(&self, steps: &mut [WorkflowStep]) -> Result<()> {
        if steps.is_empty() {
            return Ok(());
        }

        let total_batches = steps.len().div_ceil(BATCH_SIZE);

        for (batch_idx, chunk) in steps.chunks_mut(BATCH_SIZE).enumerate() {
            tracing::info!(
                "Processing AI enhancement batch {}/{} ({} steps)",
                batch_idx + 1,
                total_batches,
                chunk.len()
            );

            if let Err(e) = self.enhance_batch(chunk, batch_idx * BATCH_SIZE).await {
                tracing::warn!("Batch {} enhancement failed: {}", batch_idx + 1, e);
                // Continue with other batches even if one fails
            }

            // Small delay between batches to avoid rate limiting
            if batch_idx < total_batches - 1 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }

        Ok(())
    }

    async fn enhance_batch(&self, steps: &mut [WorkflowStep], offset: usize) -> Result<()> {
        let mut parts = vec![ContentPart::from_text(ENHANCEMENT_PROMPT.to_string())];

        // Build context for each step with screenshots
        for (i, step) in steps.iter().enumerate() {
            let step_num = offset + i + 1;

            // Add step context as text
            let step_context = format!(
                "\n\n--- Step {} ---\nAction: {:?}\nSelector: {:?}\nValue: {:?}\nAuto-generated name: {}",
                step_num,
                step.action.action_type,
                step.action.selector.as_ref().map(|s| &s.value),
                step.action.value,
                step.name
            );
            parts.push(ContentPart::from_text(step_context));

            // Add before screenshot if available
            if let Some(ref before) = step.screenshot_before {
                parts.push(ContentPart::from_text(format!(
                    "\nStep {} BEFORE screenshot:",
                    step_num
                )));
                parts.push(ContentPart::from_binary_base64(
                    "image/png",
                    before.clone(),
                    Some(format!("step_{}_before.png", step_num)),
                ));
            }

            // Add after screenshot if available
            if let Some(ref after) = step.screenshot_after {
                parts.push(ContentPart::from_text(format!(
                    "\nStep {} AFTER screenshot:",
                    step_num
                )));
                parts.push(ContentPart::from_binary_base64(
                    "image/png",
                    after.clone(),
                    Some(format!("step_{}_after.png", step_num)),
                ));
            }
        }

        // Make API request
        let request = ChatRequest::new(vec![ChatMessage::user(parts)]);
        let response = self
            .client
            .exec_chat(&self.model, request, None)
            .await
            .map_err(|e| anyhow!("AI enhancement request failed: {}", e))?;

        // Parse response and apply descriptions
        if let Some(text) = response.first_text() {
            self.parse_and_apply_descriptions(text, steps, offset)?;
        }

        Ok(())
    }

    fn parse_and_apply_descriptions(
        &self,
        response: &str,
        steps: &mut [WorkflowStep],
        offset: usize,
    ) -> Result<()> {
        // Try to extract JSON array from response
        let json_str = extract_json_array(response)
            .ok_or_else(|| anyhow!("No valid JSON array found in AI response"))?;

        let descriptions: Vec<StepDescription> = serde_json::from_str(json_str)
            .map_err(|e| anyhow!("Failed to parse AI response JSON: {}", e))?;

        // Apply descriptions to steps
        for desc in descriptions {
            let local_idx = desc.step.saturating_sub(offset + 1);
            if local_idx < steps.len() {
                steps[local_idx].description = Some(desc.description.clone());
                tracing::debug!("Applied AI description to step {}: {}", desc.step, desc.description);
            }
        }

        Ok(())
    }
}

/// Extract JSON array from response text (handles markdown code blocks)
fn extract_json_array(text: &str) -> Option<&str> {
    // Try to find JSON in markdown code block
    if let Some(start) = text.find("```json") {
        let content_start = start + 7;
        if let Some(end) = text[content_start..].find("```") {
            return Some(text[content_start..content_start + end].trim());
        }
    }

    // Try plain ``` code block
    if let Some(start) = text.find("```") {
        let content_start = start + 3;
        // Skip optional language identifier on same line
        let content_start = text[content_start..]
            .find('\n')
            .map(|n| content_start + n + 1)
            .unwrap_or(content_start);
        if let Some(end) = text[content_start..].find("```") {
            return Some(text[content_start..content_start + end].trim());
        }
    }

    // Try to find raw JSON array
    if let Some(start) = text.find('[') {
        if let Some(end) = text.rfind(']') {
            if end > start {
                return Some(&text[start..=end]);
            }
        }
    }

    None
}

const ENHANCEMENT_PROMPT: &str = r#"You are analyzing browser automation recording screenshots to generate human-readable descriptions.

For each step, you'll see:
1. The action type and technical details (selector, value)
2. A BEFORE screenshot (page state before the action)
3. An AFTER screenshot (page state after the action)

Your task: Generate clear, human-readable descriptions that help someone understand what each action does visually.

Guidelines:
- Describe the visual element being interacted with (color, position, text on it)
- Use natural language: "Click the blue 'Sign In' button in the top-right corner"
- Mention form field labels, button text, or other identifying features
- Keep descriptions concise but specific (50-100 characters)
- Focus on visual appearance, not CSS selectors

Respond ONLY with a JSON array of descriptions:
```json
[
  {"step": 1, "description": "Click the blue 'Sign In' button in the header"},
  {"step": 2, "description": "Type email address into the 'Email' input field"}
]
```

Important: Use the exact step numbers provided. Return valid JSON only."#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_array_markdown() {
        let text = r#"Here is the response:
```json
[{"step": 1, "description": "Click button"}]
```"#;
        let result = extract_json_array(text);
        assert!(result.is_some());
        assert!(result.unwrap().starts_with('['));
    }

    #[test]
    fn test_extract_json_array_raw() {
        let text = r#"[{"step": 1, "description": "Click button"}]"#;
        let result = extract_json_array(text);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), text);
    }

    #[test]
    fn test_extract_json_array_with_text() {
        let text = r#"Based on the screenshots, here are the descriptions:
[{"step": 1, "description": "Click the login button"}]
That's all!"#;
        let result = extract_json_array(text);
        assert!(result.is_some());
        assert!(result.unwrap().contains("login button"));
    }
}
