use anyhow::{anyhow, Result};
use genai::chat::{ChatMessage, ChatRequest, ContentPart, Tool, ToolResponse};
use genai::resolver::{AuthData, AuthResolver};
use genai::{Client, ModelIden};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::instrument;

use crate::agent::UserMessageBuilder;
use crate::desktop::DesktopManager;
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
    pub provider: Option<String>,
    /// Minimum delay between LLM calls in milliseconds (rate limiting)
    pub min_llm_delay_ms: u64,
}

/// Default minimum delay between LLM calls (2 seconds for rate limit safety)
const DEFAULT_MIN_LLM_DELAY_MS: u64 = 2000;

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            api_key: None,
            max_steps: MAX_STEPS,
            provider: None,
            min_llm_delay_ms: DEFAULT_MIN_LLM_DELAY_MS,
        }
    }
}

/// Run executor - manages the AI agent loop for desktop automation
pub struct RunExecutor {
    config: ExecutorConfig,
    registry: ToolRegistry,
    logger: RunLogger,
    desktop: Arc<DesktopManager>,
    /// Cancellation token for graceful shutdown
    cancel_token: CancellationToken,
}

impl RunExecutor {
    pub fn new(logger: RunLogger, desktop: Arc<DesktopManager>, config: ExecutorConfig) -> Self {
        let mut registry = ToolRegistry::new();
        register_all_tools(&mut registry);

        Self {
            config,
            registry,
            logger,
            desktop,
            cancel_token: CancellationToken::new(),
        }
    }

    /// Get the cancellation token for external cancellation
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    /// Cancel the running execution
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    /// Execute a run
    #[instrument(skip(self, run), fields(run_id = %run.id, model = %self.config.model))]
    pub async fn execute(&self, run: &Run) -> Result<()> {
        let run_id = &run.id;

        // Update status to running
        self.logger.status(run_id, RunStatus::Running, None);
        self.logger.info(run_id, "Starting desktop automation run");

        // Build the initial prompt
        let mut user_prompt = format!(
            "Task: {}",
            run.task_description
                .as_deref()
                .unwrap_or("Complete the task")
        );

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

        // Add stop condition if provided
        if let Some(stop_when) = run.metadata.get("stop_when").and_then(|v| v.as_str()) {
            if !stop_when.is_empty() {
                user_prompt.push_str(&format!(
                    r#"

CRITICAL: COMPLETION REQUIREMENT
DO NOT call the done() tool until: {}

You MUST continue working until this condition is fully satisfied.
Calling done() before meeting this requirement is a FAILURE.
Keep taking actions until the condition above is clearly met.
"#,
                    stop_when
                ));
            }
        }

        // Create in-memory storage for memories/notes
        let memories = Arc::new(RwLock::new(Vec::new()));

        // Create tool context
        let ctx = ToolContext {
            run_id: run_id.clone(),
            workflow_id: run.workflow_id.clone(),
            desktop: Arc::clone(&self.desktop),
            memories: Arc::clone(&memories),
        };

        // Create genai client
        let client = if let Some(api_key) = &self.config.api_key {
            let api_key = api_key.clone();
            let auth_resolver = AuthResolver::from_resolver_fn(
                move |_model_iden: ModelIden| -> std::result::Result<Option<AuthData>, genai::resolver::Error> {
                    Ok(Some(AuthData::from_single(api_key.clone())))
                },
            );
            Client::builder()
                .with_auth_resolver(auth_resolver)
                .build()
        } else {
            Client::default()
        };

        // Convert our tools to genai tools
        let tools = self.build_genai_tools();

        // Keep conversation history WITHOUT screenshots (text only) to save tokens
        // Only the CURRENT desktop state gets a screenshot
        let mut history: Vec<ChatMessage> = vec![ChatMessage::system(SYSTEM_PROMPT)];

        // Add initial user message (text only for history)
        history.push(ChatMessage::user(user_prompt.clone()));

        let mut step_number = 0;
        let mut first_iteration = true;
        let mut last_llm_call: Option<Instant> = None;

        // Get max_steps from run metadata or use config default
        let max_steps = run
            .metadata
            .get("max_steps")
            .and_then(|v| v.as_i64())
            .map(|v| v as usize)
            .unwrap_or(self.config.max_steps);

        // Agent loop
        loop {
            // Check for cancellation
            if self.cancel_token.is_cancelled() {
                self.logger.info(run_id, "Run cancelled by user");
                self.logger
                    .status(run_id, RunStatus::Cancelled, Some("Cancelled by user".to_string()));
                return Ok(());
            }

            // Check for pause (wait until resumed)
            while self.desktop.is_paused() {
                if self.cancel_token.is_cancelled() {
                    self.logger.info(run_id, "Run cancelled while paused");
                    self.logger
                        .status(run_id, RunStatus::Cancelled, Some("Cancelled while paused".to_string()));
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }

            if step_number >= max_steps {
                self.logger
                    .warn(run_id, format!("Reached maximum steps limit ({})", max_steps));
                self.logger.status(
                    run_id,
                    RunStatus::Completed,
                    Some("Completed (reached step limit)".to_string()),
                );
                break;
            }

            // Build fresh request: history (text-only) + current desktop state (WITH screenshot)
            let chat_req = if first_iteration {
                // First iteration: capture desktop state + use initial prompt
                let state = self.desktop.capture_state().map_err(|e| {
                    anyhow!("Failed to capture initial desktop state: {}", e)
                })?;

                // Build request with initial prompt + screenshot
                let text = format!("{}\n\n{}", user_prompt, format_desktop_context(&state.display_info, &state.active_window, &state.elements_text, 0, max_steps));
                self.build_request_with_screenshot(&history, &text, Some(state.screenshot_base64), &tools)
            } else {
                // Subsequent iterations: get current desktop state + memories
                let state = self.desktop.capture_state().map_err(|e| {
                    anyhow!("Failed to capture desktop state: {}", e)
                })?;

                let memories_snapshot = memories.read().await;

                let text = UserMessageBuilder::new()
                    .with_memories(&memories_snapshot)
                    .with_desktop_state(
                        &state.display_info,
                        &state.active_window,
                        &state.elements_text,
                    )
                    .with_step_info(step_number, max_steps)
                    .build();

                self.build_request_with_screenshot(&history, &text, Some(state.screenshot_base64), &tools)
            };
            first_iteration = false;

            // Log what we're sending
            let step_log = format!(
                "Step {}/{} | Sending {} messages to LLM",
                step_number + 1,
                max_steps,
                history.len() + 1
            );
            tracing::info!("{}", step_log);
            self.logger.info(run_id, step_log);

            // Rate limiting
            if let Some(last_call) = last_llm_call {
                let elapsed = last_call.elapsed();
                let min_delay = Duration::from_millis(self.config.min_llm_delay_ms);
                if elapsed < min_delay {
                    let sleep_time = min_delay - elapsed;
                    self.logger.debug(
                        run_id,
                        format!("Rate limiting: sleeping {}ms", sleep_time.as_millis()),
                    );
                    tokio::time::sleep(sleep_time).await;
                }
            }
            last_llm_call = Some(Instant::now());

            // Call the model with retry logic
            const MAX_RETRIES: u32 = 5;
            const INITIAL_BACKOFF_MS: u64 = 2000;

            let llm_response: genai::chat::ChatResponse = 'retry: {
                let mut retry_count = 0u32;
                let mut backoff_ms = INITIAL_BACKOFF_MS;

                loop {
                    let result = tokio::time::timeout(
                        Duration::from_secs(120),
                        client.exec_chat(&self.config.model, chat_req.clone(), None),
                    )
                    .await
                    .map_err(|_| anyhow!("LLM request timeout after 120s"))?
                    .map_err(|e| anyhow!("{}", e));

                    match result {
                        Ok(res) => break 'retry res,
                        Err(e) => {
                            let error_str = e.to_string();
                            let is_rate_limit = error_str.contains("429")
                                || error_str.contains("RESOURCE_EXHAUSTED")
                                || error_str.contains("Too Many Requests")
                                || error_str.contains("rate limit");

                            if is_rate_limit && retry_count < MAX_RETRIES {
                                retry_count += 1;
                                self.logger.info(
                                    run_id,
                                    format!(
                                        "Rate limited (429). Retry {}/{} after {}ms backoff...",
                                        retry_count, MAX_RETRIES, backoff_ms
                                    ),
                                );
                                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                                backoff_ms *= 2;
                                last_llm_call = Some(Instant::now());
                                continue;
                            }

                            let error = if is_rate_limit {
                                format!(
                                    "LLM request failed after {} retries (rate limited): {}",
                                    retry_count, e
                                )
                            } else {
                                format!("LLM request failed: {}", e)
                            };
                            self.logger.error(run_id, &error);
                            self.logger
                                .status(run_id, RunStatus::Failed, Some(error.clone()));
                            return Err(anyhow!(error));
                        }
                    }
                }
            };

            // Extract text content before consuming
            let text_content = llm_response.first_text().map(|s| s.to_string());

            // Check for tool calls
            let tool_calls = llm_response.into_tool_calls();

            if tool_calls.is_empty() {
                if let Some(content) = text_content {
                    self.logger
                        .info(run_id, "LLM returned text response, completing task");
                    self.logger.result(run_id, &content);
                } else {
                    self.logger
                        .info(run_id, "No tool calls in response, task may be complete");
                }
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

                // Resolve variables in parameters
                let resolved_params = resolve_variables(&params, &variables);

                // Log tool call
                let params_str = serde_json::to_string(&resolved_params).unwrap_or_default();
                let params_short: String = params_str.chars().take(80).collect();
                let tool_log = format!("{}({})", tool_name, params_short);
                tracing::info!("{}", tool_log);
                self.logger.info(run_id, tool_log);

                // Create step record
                let mut step = RunStep::new(
                    run_id.clone(),
                    step_number as i32,
                    tool_name.clone(),
                    params.clone(),
                );
                self.logger.step(&step);

                let start = std::time::Instant::now();

                // Execute the tool
                let result = match self.registry.execute(tool_name, resolved_params, &ctx).await {
                    Ok(r) => r,
                    Err(e) => ToolResult::error(format!("Tool execution error: {}", e)),
                };

                let duration_ms = start.elapsed().as_millis() as i64;

                // Log result
                let result_log = format!(
                    "{} -> {} ({}ms)",
                    tool_name,
                    if result.success { "success" } else { "failed" },
                    duration_ms
                );
                tracing::info!("{}", result_log);
                self.logger.info(run_id, result_log);

                // Update step with result
                step.complete(
                    result.success,
                    result.content.clone(),
                    result.error.clone(),
                    duration_ms,
                );

                // Capture screenshot after action (for all tools that modify the desktop)
                if is_desktop_action_tool(tool_name) {
                    // Small delay to let the UI update after the action
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    if let Ok(state) = self.desktop.capture_state() {
                        step.screenshot = Some(state.raw_screenshot_base64);
                    }
                }

                self.logger.update_step(&step);

                if result.success {
                    self.logger.debug(
                        run_id,
                        format!("Tool {} succeeded: {:?}", tool_name, result.content),
                    );
                } else {
                    self.logger.warn(
                        run_id,
                        format!("Tool {} failed: {:?}", tool_name, result.error),
                    );
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

            // Add to history (text only, no screenshots)
            let genai_tool_calls: Vec<genai::chat::ToolCall> = tool_calls
                .iter()
                .map(|tc| genai::chat::ToolCall {
                    call_id: tc.call_id.clone(),
                    fn_name: tc.fn_name.clone(),
                    fn_arguments: tc.fn_arguments.clone(),
                })
                .collect();
            history.push(ChatMessage::from(genai_tool_calls));

            for response in tool_responses {
                history.push(ChatMessage::from(response));
            }

            // Sliding window: keep system + initial prompt + last ~10 steps
            const MAX_HISTORY_MESSAGES: usize = 32;
            if history.len() > MAX_HISTORY_MESSAGES {
                let to_remove = history.len() - MAX_HISTORY_MESSAGES;
                history.drain(2..2 + to_remove);
                tracing::debug!("Trimmed {} old messages from history", to_remove);
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

    /// Build a ChatRequest from history + current message with screenshot
    fn build_request_with_screenshot(
        &self,
        history: &[ChatMessage],
        current_text: &str,
        screenshot: Option<String>,
        tools: &[Tool],
    ) -> ChatRequest {
        let mut req = ChatRequest::new(history.to_vec()).with_tools(tools.to_vec());

        let mut parts = vec![ContentPart::from_text(current_text)];
        if let Some(screenshot_base64) = screenshot {
            parts.push(ContentPart::from_binary_base64(
                "image/jpeg",
                screenshot_base64,
                Some("screenshot.jpg".to_string()),
            ));
        }
        req = req.append_message(ChatMessage::user(parts));
        req
    }
}

/// Format a simple desktop context string (used for initial prompt)
fn format_desktop_context(
    display_info: &str,
    active_window: &str,
    elements_text: &str,
    step: usize,
    max_steps: usize,
) -> String {
    format!(
        "<desktop_state>\nScreen: {}\nActive window: \"{}\"\nStep: {}/{}\n\nInteractive Elements:\n{}\n\nUse click_element(index) for precise clicks. Use desktop_click(x, y) only if the element is not in the list above.\n</desktop_state>",
        display_info, active_window, step, max_steps, elements_text
    )
}

/// Check if a tool modifies the desktop (for screenshot capture after action)
fn is_desktop_action_tool(name: &str) -> bool {
    matches!(
        name,
        "click_element"
            | "input_text"
            | "desktop_click"
            | "desktop_type"
            | "desktop_key"
            | "desktop_scroll"
            | "desktop_drag"
            | "open_application"
            | "focus_window"
    )
}

/// Replace {{variable_name}} placeholders in JSON params with actual values
fn resolve_variables(params: &Value, variables: &HashMap<String, String>) -> Value {
    if variables.is_empty() {
        return params.clone();
    }
    resolve_variables_recursive(params, variables)
}

fn resolve_variables_recursive(value: &Value, variables: &HashMap<String, String>) -> Value {
    match value {
        Value::String(s) => {
            let mut result = s.clone();
            for (name, var_value) in variables {
                let pattern = format!("{{{{{}}}}}", name);
                result = result.replace(&pattern, var_value);
            }
            Value::String(result)
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), resolve_variables_recursive(v, variables)))
                .collect(),
        ),
        Value::Array(arr) => Value::Array(
            arr.iter()
                .map(|v| resolve_variables_recursive(v, variables))
                .collect(),
        ),
        _ => value.clone(),
    }
}
