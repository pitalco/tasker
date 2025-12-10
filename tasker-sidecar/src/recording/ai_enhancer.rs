//! AI-powered enhancement for recorded workflow steps
//! Uses vision models to generate human-readable descriptions from screenshots

use anyhow::{anyhow, Result};
use genai::chat::{ChatMessage, ChatRequest, ContentPart};
use genai::Client;
use serde_json::{json, Value};

use crate::config;
use crate::llm::{RailwayClient, RailwayChatRequest, RailwayMessage};
use crate::models::WorkflowStep;


/// Result of task description generation
#[derive(Debug)]
pub struct TaskDescriptionResult {
    pub name: String,
    pub description: String,
}

/// Provider type for AI enhancement
enum EnhancerProvider {
    /// Use genai Client with API key
    Genai { client: Client, model: String },
    /// Use Tasker Fast via Railway proxy
    TaskerFast { client: RailwayClient },
}

/// AI enhancement service for recorded workflows
pub struct AIEnhancer {
    provider: EnhancerProvider,
}

impl AIEnhancer {
    /// Create a new AI enhancer
    /// - auth_token: Required for Tasker Fast provider
    pub fn new(auth_token: Option<String>) -> Option<Self> {
        // Check default provider from settings
        let default_provider = config::get_default_provider()
            .unwrap_or_else(|| "gemini".to_string());

        tracing::info!("AI enhancer: provider='{}', has_auth_token={}",
            default_provider, auth_token.is_some());

        if default_provider == "tasker-fast" {
            // Use Tasker Fast - requires auth token
            if auth_token.is_none() {
                tracing::error!("Tasker Fast selected but no auth token provided!");
                return None;
            }
            let auth_token = auth_token.unwrap();
            tracing::info!("Using Tasker Fast for AI enhancement");
            Some(Self {
                provider: EnhancerProvider::TaskerFast {
                    client: RailwayClient::with_token(auth_token),
                },
            })
        } else {
            // Get model from settings - required, no fallbacks
            let model_name = config::get_default_model();
            if model_name.is_none() {
                tracing::warn!("No default model configured in settings");
                return None;
            }
            let model_name = model_name.unwrap();

            // Determine provider from the configured default
            let (provider_name, env_var) = if default_provider.starts_with("claude") || default_provider == "anthropic" {
                ("anthropic", "ANTHROPIC_API_KEY")
            } else if default_provider.starts_with("gpt") || default_provider == "openai" {
                ("openai", "OPENAI_API_KEY")
            } else if default_provider.starts_with("gemini") || default_provider == "google" {
                ("gemini", "GEMINI_API_KEY")
            } else {
                tracing::warn!("Unknown provider: {} - configure a valid provider in settings", default_provider);
                return None;
            };

            // Load API key from app settings database
            let api_key = config::get_api_key(provider_name);
            if api_key.is_none() {
                tracing::warn!("No API key found for {} - configure it in settings", provider_name);
                return None;
            }

            let api_key = api_key.unwrap();
            unsafe { std::env::set_var(env_var, api_key) };
            tracing::info!("Using {} model {} for AI enhancement", provider_name, model_name);

            Some(Self {
                provider: EnhancerProvider::Genai {
                    client: Client::default(),
                    model: model_name,
                },
            })
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

        let response_text = match &self.provider {
            EnhancerProvider::Genai { client, model } => {
                self.generate_with_genai(client, model, steps, start_url).await?
            }
            EnhancerProvider::TaskerFast { client } => {
                self.generate_with_tasker_fast(client, steps, start_url).await?
            }
        };

        // Parse name and description from response
        let result = Self::parse_response(&response_text);

        tracing::info!("Generated task: '{}' ({} chars)", result.name, result.description.len());

        Ok(result)
    }

    /// Generate task description using genai client (Gemini/OpenAI/Anthropic)
    async fn generate_with_genai(
        &self,
        client: &Client,
        model: &str,
        steps: &[WorkflowStep],
        start_url: &str,
    ) -> Result<String> {
        let mut parts = vec![ContentPart::from_text(TASK_DESCRIPTION_PROMPT.to_string())];

        // Add start URL context
        parts.push(ContentPart::from_text(format!(
            "\n\n=== RECORDING SESSION ===\nStart URL: {}\nTotal Steps: {}\n",
            start_url,
            steps.len()
        )));

        // Build context for each step with screenshots
        // Only include screenshots for first 3 and last 3 steps to avoid request size limits
        let total_steps = steps.len();
        for (i, step) in steps.iter().enumerate() {
            let step_num = i + 1;
            let include_screenshot = i < 3 || i >= total_steps.saturating_sub(3);

            // Add step context as text
            let step_context = format!(
                "\n\n--- Step {} of {} ---\nAction: {:?}\nSelector: {:?}\nValue: {:?}",
                step_num,
                total_steps,
                step.action.action_type,
                step.action.selector.as_ref().map(|s| format!("{:?}: {}", s.strategy, s.value)),
                step.action.value
            );
            parts.push(ContentPart::from_text(step_context));

            // Add screenshots only for first/last steps to keep request size manageable
            if include_screenshot {
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
        }

        // Make API request
        let request = ChatRequest::new(vec![ChatMessage::user(parts)]);
        let response = client
            .exec_chat(model, request, None)
            .await
            .map_err(|e| anyhow!("AI task description generation failed: {}", e))?;

        Ok(response
            .first_text()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Failed to generate task description.".to_string()))
    }

    /// Generate task description using Tasker Fast (Railway proxy)
    async fn generate_with_tasker_fast(
        &self,
        client: &RailwayClient,
        steps: &[WorkflowStep],
        start_url: &str,
    ) -> Result<String> {
        // Build multimodal content array in OpenAI format
        let mut content_parts: Vec<Value> = vec![];

        // Add prompt
        content_parts.push(json!({
            "type": "text",
            "text": TASK_DESCRIPTION_PROMPT
        }));

        // Add start URL context
        content_parts.push(json!({
            "type": "text",
            "text": format!(
                "\n\n=== RECORDING SESSION ===\nStart URL: {}\nTotal Steps: {}\n",
                start_url,
                steps.len()
            )
        }));

        // Build context for each step with screenshots
        // Only include screenshots for first 3 and last 3 steps to avoid request size limits
        let total_steps = steps.len();
        for (i, step) in steps.iter().enumerate() {
            let step_num = i + 1;
            let include_screenshot = i < 3 || i >= total_steps.saturating_sub(3);

            // Add step context as text
            let step_context = format!(
                "\n\n--- Step {} of {} ---\nAction: {:?}\nSelector: {:?}\nValue: {:?}",
                step_num,
                total_steps,
                step.action.action_type,
                step.action.selector.as_ref().map(|s| format!("{:?}: {}", s.strategy, s.value)),
                step.action.value
            );
            content_parts.push(json!({
                "type": "text",
                "text": step_context
            }));

            // Add screenshots only for first/last steps to keep request size manageable
            if include_screenshot {
                // Add before screenshot if available
                if let Some(ref before) = step.screenshot_before {
                    content_parts.push(json!({
                        "type": "text",
                        "text": format!("\nStep {} BEFORE screenshot:", step_num)
                    }));
                    content_parts.push(json!({
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{}", before)
                        }
                    }));
                }

                // Add after screenshot if available
                if let Some(ref after) = step.screenshot_after {
                    content_parts.push(json!({
                        "type": "text",
                        "text": format!("\nStep {} AFTER screenshot:", step_num)
                    }));
                    content_parts.push(json!({
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{}", after)
                        }
                    }));
                }
            }
        }

        // Create request
        let request = RailwayChatRequest {
            messages: vec![RailwayMessage {
                role: "user".to_string(),
                content: Value::Array(content_parts),
                tool_calls: None,
                tool_call_id: None,
            }],
            max_tokens: Some(4096),
            temperature: Some(0.7),
            tools: None,
            tool_choice: None,
        };

        let response = client
            .chat(request)
            .await
            .map_err(|e| anyhow!("AI task description generation failed: {}", e))?;

        response
            .choices
            .first()
            .and_then(|c| c.message.content.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("No text in response"))
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

const TASK_DESCRIPTION_PROMPT: &str = r#"You are analyzing a recorded browser automation session. Your task is to generate a reusable task description that a COMPUTER USE AI AGENT will follow to replicate this workflow.

IMPORTANT CONTEXT:
- The AI agent uses tools like click_element, input_text, go_to_url to interact with a browser
- The agent sees both screenshots AND structured element data to identify targets
- The description must be GENERIC and REUSABLE - use placeholders like {{variable_name}} for any user-specific data

You will see:
1. The starting URL
2. Each action with its type, selector, and value
3. BEFORE and AFTER screenshots showing state changes

Your response MUST use this exact format:

<name>Short descriptive name (3-6 words)</name>
<description>
Navigate to [URL]

1. First action description
2. Second action description
...
</description>

Guidelines for the name:
- Keep it short (3-6 words)
- Describe the main goal (e.g., "Login to Dashboard", "Submit Contact Form", "Google Search")

Guidelines for the description:
- Start with the URL to navigate to
- Describe each action clearly and concisely
- Use {{variable_name}} placeholders for user-specific data that should be configurable:
  - {{email}}, {{password}}, {{search_query}}, {{username}}, etc.
- If a navigation happened without a visible click (URL bar), describe it as "Navigate to [URL]"

Example response:

<name>Login to Admin Dashboard</name>
<description>
Navigate to https://example.com/admin

1. Click the "Sign In" button
2. Enter {{email}} in the email field
3. Enter {{password}} in the password field
4. Click the "Log In" button
5. Verify the dashboard loads
</description>

Generate the name and description now:"#;
