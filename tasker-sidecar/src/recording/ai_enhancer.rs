//! AI-powered enhancement for recorded workflow steps
//! Uses vision models to generate human-readable descriptions from screenshots

use anyhow::{anyhow, Result};
use genai::chat::{ChatMessage, ChatRequest, ContentPart};
use genai::Client;

use crate::config;
use crate::models::WorkflowStep;

/// Default model for AI enhancement (vision-capable)
const DEFAULT_ENHANCEMENT_MODEL: &str = "gemini-2.5-flash";

/// Result of task description generation
#[derive(Debug)]
pub struct TaskDescriptionResult {
    pub name: String,
    pub description: String,
}

/// AI enhancement service for recorded workflows
pub struct AIEnhancer {
    client: Client,
    model: String,
}

impl AIEnhancer {
    pub fn new(model: Option<&str>) -> Self {
        let model_name = model.unwrap_or(DEFAULT_ENHANCEMENT_MODEL);

        // Determine provider from model name and load API key from app settings
        let provider = if model_name.starts_with("gemini") {
            "gemini"
        } else if model_name.starts_with("gpt") {
            "openai"
        } else if model_name.starts_with("claude") {
            "anthropic"
        } else {
            "gemini" // Default to gemini
        };

        // Load API key from app settings database
        if let Some(api_key) = config::get_api_key(provider) {
            let env_var = match provider {
                "anthropic" => "ANTHROPIC_API_KEY",
                "openai" => "OPENAI_API_KEY",
                _ => "GEMINI_API_KEY",
            };
            std::env::set_var(env_var, api_key);
            tracing::debug!("Loaded {} API key from app settings", provider);
        }

        Self {
            client: Client::default(),
            model: model_name.to_string(),
        }
    }

    /// Generate a comprehensive task description from all recorded steps
    ///
    /// Analyzes all screenshots and actions to produce a detailed description
    /// that an AI agent can use to replicate the workflow.
    /// Returns both a short name and the full task description.
    pub async fn generate_task_description(&self, steps: &[WorkflowStep], start_url: &str) -> Result<TaskDescriptionResult> {
        if steps.is_empty() {
            return Ok(TaskDescriptionResult {
                name: "Empty Recording".to_string(),
                description: "No actions were recorded.".to_string(),
            });
        }

        tracing::info!("Generating task description from {} steps", steps.len());

        let mut parts = vec![ContentPart::from_text(TASK_DESCRIPTION_PROMPT.to_string())];

        // Add start URL context
        parts.push(ContentPart::from_text(format!(
            "\n\n=== RECORDING SESSION ===\nStart URL: {}\nTotal Steps: {}\n",
            start_url,
            steps.len()
        )));

        // Build context for each step with screenshots
        for (i, step) in steps.iter().enumerate() {
            let step_num = i + 1;

            // Add step context as text
            let step_context = format!(
                "\n\n--- Step {} of {} ---\nAction: {:?}\nSelector: {:?}\nValue: {:?}",
                step_num,
                steps.len(),
                step.action.action_type,
                step.action.selector.as_ref().map(|s| format!("{:?}: {}", s.strategy, s.value)),
                step.action.value
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
            .map_err(|e| anyhow!("AI task description generation failed: {}", e))?;

        // Extract the response text
        let response_text = response
            .first_text()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Failed to generate task description.".to_string());

        // Parse name and description from response
        let result = Self::parse_response(&response_text);

        tracing::info!("Generated task: '{}' ({} chars)", result.name, result.description.len());

        Ok(result)
    }

    /// Parse the AI response to extract name and description
    fn parse_response(response: &str) -> TaskDescriptionResult {
        let mut name = String::new();
        let mut description = String::new();
        let mut in_description = false;

        for line in response.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("<name>") && trimmed.ends_with("</name>") {
                // Single line: <name>Login to Dashboard</name>
                name = trimmed
                    .trim_start_matches("<name>")
                    .trim_end_matches("</name>")
                    .trim()
                    .to_string();
            } else if trimmed.starts_with("<name>") {
                // Multi-line start
                name = trimmed.trim_start_matches("<name>").trim().to_string();
            } else if trimmed.ends_with("</name>") {
                // Multi-line end
                if !name.is_empty() {
                    name.push(' ');
                }
                name.push_str(trimmed.trim_end_matches("</name>").trim());
            } else if trimmed == "<description>" {
                in_description = true;
            } else if trimmed == "</description>" {
                in_description = false;
            } else if in_description {
                if !description.is_empty() {
                    description.push('\n');
                }
                description.push_str(line);
            }
        }

        // Fallback if parsing fails
        if name.is_empty() {
            name = "Recorded Workflow".to_string();
        }
        if description.is_empty() {
            description = response.to_string();
        }

        TaskDescriptionResult { name, description }
    }
}

const TASK_DESCRIPTION_PROMPT: &str = r#"You are analyzing a recorded browser automation session. Generate a name and detailed task description that an AI agent could use to replicate this workflow.

You will see:
1. The starting URL
2. Each action with its type, selector, and value
3. BEFORE and AFTER screenshots for each step

Your response MUST use this exact format:

<name>Short descriptive name (3-6 words)</name>
<description>
Navigate to [URL]

1. First action with specific details
2. Second action with specific details
...
</description>

Guidelines for the name:
- Keep it short (3-6 words)
- Describe the main goal (e.g., "Login to Dashboard", "Submit Contact Form", "Search for Product")

Guidelines for the description:
- Start with the URL to navigate to
- Describe each action in clear, step-by-step detail
- Use specific visual identifiers (button text, input labels, colors, positions)
- Include any text that was typed or options that were selected
- Be detailed enough for an AI agent to replicate without seeing the screenshots

Example response:

<name>Login to Admin Dashboard</name>
<description>
Navigate to https://example.com/admin

1. Click the "Sign In" button in the top-right corner of the navigation bar
2. In the email input field (labeled "Email Address"), type the email address
3. In the password field below the email field, type the password
4. Click the blue "Log In" button below the password field
5. Wait for the dashboard to load
</description>

Generate the name and description now:"#;
