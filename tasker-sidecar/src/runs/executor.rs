use anyhow::{anyhow, Result};
use genai::chat::{ChatMessage, ChatRequest, ContentPart, Tool, ToolResponse};
use genai::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::agent::UserMessageBuilder;
use crate::browser::{BrowserManager, SelectorMap};
use crate::tools::{register_all_tools, ToolContext, ToolRegistry, ToolResult};

use super::logger::RunLogger;
use super::models::{Run, RunStatus, RunStep};

use crate::llm::prompts::SYSTEM_PROMPT;

const MAX_STEPS: usize = 50;
const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

/// Configuration for a run execution
pub struct ExecutorConfig {
    pub model: String,
    pub api_key: Option<String>,
    pub max_steps: usize,
    pub headless: bool,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            api_key: None,
            max_steps: MAX_STEPS,
            headless: false,
        }
    }
}

/// Run executor - manages the AI agent loop
pub struct RunExecutor {
    config: ExecutorConfig,
    registry: ToolRegistry,
    logger: RunLogger,
    browser: Arc<BrowserManager>,
}

impl RunExecutor {
    pub fn new(logger: RunLogger, browser: Arc<BrowserManager>, config: ExecutorConfig) -> Self {
        let mut registry = ToolRegistry::new();
        register_all_tools(&mut registry);

        Self {
            config,
            registry,
            logger,
            browser,
        }
    }

    /// Execute a run
    pub async fn execute(&self, run: &Run) -> Result<()> {
        let run_id = &run.id;

        // Update status to running
        self.logger.status(run_id, RunStatus::Running, None);
        self.logger.info(run_id, "Starting run execution");

        // Build the initial prompt
        let mut user_prompt = format!("Task: {}", run.task_description.as_deref().unwrap_or("Complete the workflow"));

        // Add workflow hints if available
        if let Some(hints) = &run.metadata.get("hints") {
            user_prompt.push_str(&format!("\n\nWorkflow hints (use as guidance):\n{}", hints));
        }

        // Add custom instructions if provided
        if let Some(instructions) = &run.custom_instructions {
            user_prompt.push_str(&format!("\n\nAdditional instructions:\n{}", instructions));
        }

        // Extract variables from metadata for use in tool parameter substitution
        let variables: HashMap<String, String> = run
            .metadata
            .get("variables")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // Add variable names to prompt if any exist
        if !variables.is_empty() {
            let var_names: Vec<String> = variables.keys().map(|k| format!("- {}", k)).collect();
            user_prompt.push_str(&format!(
                "\n\n<variables>\nAvailable variables (use {{{{variable_name}}}} syntax):\n{}\n</variables>",
                var_names.join("\n")
            ));
        }

        // Add stop condition if provided (AGGRESSIVE)
        if let Some(stop_when) = run.metadata.get("stop_when").and_then(|v| v.as_str()) {
            if !stop_when.is_empty() {
                user_prompt.push_str(&format!(r#"

⚠️ CRITICAL: COMPLETION REQUIREMENT ⚠️
════════════════════════════════════════
DO NOT call the done() tool until: {}

You MUST continue working until this condition is fully satisfied.
Calling done() before meeting this requirement is a FAILURE.
Keep taking actions until the condition above is clearly met.
════════════════════════════════════════
"#, stop_when));
            }
        }

        // Create selector map storage (will be updated before each LLM call)
        let selector_map = Arc::new(RwLock::new(SelectorMap::new()));

        // Create tool context with file repository access
        let ctx = ToolContext {
            run_id: run_id.clone(),
            workflow_id: run.workflow_id.clone(),
            browser: Arc::clone(&self.browser),
            selector_map: Arc::clone(&selector_map),
            file_repository: Some(Arc::new(self.logger.repository().clone())),
        };

        // Set up API key if provided
        if let Some(ref api_key) = self.config.api_key {
            std::env::set_var("ANTHROPIC_API_KEY", api_key);
        }

        // Create genai client
        let client = Client::default();

        // Convert our tools to genai tools
        let tools = self.build_genai_tools();

        // Build initial chat request with system prompt
        let mut chat_req = ChatRequest::new(vec![ChatMessage::system(SYSTEM_PROMPT)])
            .with_tools(tools);

        // Add initial user message with task and screenshot
        let initial_message = self.build_user_message_with_screenshot(&user_prompt).await;
        chat_req = chat_req.append_message(initial_message);

        let mut step_number = 0;
        let mut first_iteration = true;

        // Get max_steps from run metadata (workflow override) or use config default
        let max_steps = run
            .metadata
            .get("max_steps")
            .and_then(|v| v.as_i64())
            .map(|v| v as usize)
            .unwrap_or(self.config.max_steps);

        // Agent loop
        loop {
            if step_number >= max_steps {
                self.logger.warn(run_id, format!("Reached maximum steps limit ({})", max_steps));
                self.logger.status(
                    run_id,
                    RunStatus::Completed,
                    Some("Completed (reached step limit)".to_string()),
                );
                break;
            }

            // For subsequent iterations, take fresh screenshot and add page state RIGHT BEFORE LLM call
            if !first_iteration {
                let page_state = self.build_page_state_message(&selector_map).await;
                chat_req = chat_req.append_message(page_state);
            }
            first_iteration = false;

            self.logger.debug(run_id, format!("Step {}: Calling LLM", step_number + 1));

            // Call the model
            let chat_res = match client.exec_chat(&self.config.model, chat_req.clone(), None).await {
                Ok(res) => res,
                Err(e) => {
                    let error = format!("LLM request failed: {}", e);
                    self.logger.error(run_id, &error);
                    self.logger.status(run_id, RunStatus::Failed, Some(error));
                    return Err(anyhow!("LLM request failed: {}", e));
                }
            };

            // Check for tool calls
            let tool_calls = chat_res.into_tool_calls();

            if tool_calls.is_empty() {
                // No tool calls - model might have responded with text
                self.logger.info(run_id, "No tool calls in response, task may be complete");
                self.logger.status(run_id, RunStatus::Completed, None);
                break;
            }

            // Process each tool call
            let mut tool_responses = Vec::new();
            let mut is_done = false;

            for tool_call in &tool_calls {
                step_number += 1;

                let tool_name = &tool_call.fn_name;
                let params: Value = tool_call.fn_arguments.clone();

                // Resolve variables in parameters before execution
                let resolved_params = resolve_variables(&params, &variables);

                self.logger.info(run_id, format!("Executing tool: {} with params: {}", tool_name, resolved_params));

                // Create step record (store original params for logging, use resolved for execution)
                let mut step = RunStep::new(
                    run_id.clone(),
                    step_number as i32,
                    tool_name.clone(),
                    params.clone(),
                );

                // Persist step to DB (no broadcast until complete)
                self.logger.step(&step);

                let start = std::time::Instant::now();

                // Execute the tool with resolved parameters
                let result = match self.registry.execute(tool_name, resolved_params, &ctx).await {
                    Ok(r) => r,
                    Err(e) => ToolResult::error(format!("Tool execution error: {}", e)),
                };

                let duration_ms = start.elapsed().as_millis() as i64;

                // Update step with result
                step.complete(
                    result.success,
                    result.content.clone(),
                    result.error.clone(),
                    duration_ms,
                );

                // Take screenshot after action (if browser tool)
                if is_browser_tool(tool_name) {
                    if let Ok(screenshot) = self.browser.screenshot().await {
                        step.screenshot = Some(screenshot);
                    }
                }

                // Log step completion
                self.logger.update_step(&step);

                // Log result
                if result.success {
                    self.logger.debug(run_id, format!("Tool {} succeeded: {:?}", tool_name, result.content));
                } else {
                    self.logger.warn(run_id, format!("Tool {} failed: {:?}", tool_name, result.error));
                }

                // Build tool response
                let response_content = if result.success {
                    json!({
                        "success": true,
                        "content": result.content,
                        "data": result.data
                    })
                } else {
                    json!({
                        "success": false,
                        "error": result.error
                    })
                };

                tool_responses.push(ToolResponse::new(
                    tool_call.call_id.clone(),
                    response_content.to_string(),
                ));

                // Check if done
                if result.is_done {
                    is_done = true;
                    let status = if result.success {
                        RunStatus::Completed
                    } else {
                        RunStatus::Failed
                    };

                    // Save the final result/response from the agent
                    if let Some(ref content) = result.content {
                        self.logger.result(run_id, content);
                    }

                    self.logger.status(run_id, status, result.error.clone());
                }
            }

            if is_done {
                self.logger.info(run_id, "Task completed via done tool");
                break;
            }

            // Append tool calls and responses to chat history
            // (Page state with screenshot will be added at START of next iteration)
            chat_req = chat_req.append_message(tool_calls);
            for response in tool_responses {
                chat_req = chat_req.append_message(response);
            }
        }

        Ok(())
    }

    /// Convert our tool definitions to genai Tool format
    fn build_genai_tools(&self) -> Vec<Tool> {
        self.registry
            .definitions()
            .into_iter()
            .map(|def| {
                Tool::new(&def.name)
                    .with_description(&def.description)
                    .with_schema(def.parameters)
            })
            .collect()
    }

    /// Build a user message with text and optional screenshot
    async fn build_user_message_with_screenshot(&self, text: &str) -> ChatMessage {
        let mut parts = vec![ContentPart::from_text(text)];

        // Try to take a screenshot
        if let Ok(screenshot_base64) = self.browser.screenshot().await {
            // Add screenshot as binary content
            parts.push(ContentPart::from_binary_base64(
                "image/png",
                screenshot_base64,
                Some("screenshot.png".to_string()),
            ));
        }

        ChatMessage::user(parts)
    }

    /// Build a follow-up message with current page state using UserMessageBuilder
    async fn build_page_state_message(&self, selector_map: &Arc<RwLock<SelectorMap>>) -> ChatMessage {
        let url = self.browser.current_url().await.unwrap_or_default();
        let title = self.browser.get_title().await.unwrap_or_default();

        // Get DOM extraction result from page
        let dom_result = self.browser.get_indexed_elements().await.unwrap_or_default();

        // Update the shared selector map for tools
        *selector_map.write().await = dom_result.selector_map.clone();

        // Use UserMessageBuilder for formatting
        let text = UserMessageBuilder::new()
            .with_browser_state(&url, &title, &dom_result)
            .build();

        self.build_user_message_with_screenshot(&text).await
    }
}

/// Check if a tool interacts with the browser (for screenshot capture)
fn is_browser_tool(name: &str) -> bool {
    matches!(
        name,
        "search_google"
            | "go_to_url"
            | "go_back"
            | "click_element"
            | "input_text"
            | "scroll_down"
            | "scroll_up"
            | "send_keys"
            | "select_dropdown_option"
            | "execute_javascript"
    )
}

/// Replace {{variable_name}} placeholders in JSON params with actual values
fn resolve_variables(params: &Value, variables: &HashMap<String, String>) -> Value {
    if variables.is_empty() {
        return params.clone();
    }

    let mut json_str = params.to_string();
    for (name, value) in variables {
        let pattern = format!("{{{{{}}}}}", name); // {{name}}
        json_str = json_str.replace(&pattern, value);
    }

    serde_json::from_str(&json_str).unwrap_or_else(|_| params.clone())
}
