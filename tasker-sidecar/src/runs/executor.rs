use anyhow::{anyhow, Result};
use genai::chat::{ChatMessage, ChatRequest, ContentPart, MessageContent, Tool, ToolResponse};
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
use crate::browser::{BrowserManager, SelectorMap};
use crate::tools::{register_all_tools, RunSpawner, TerminalSession, ToolContext, ToolRegistry, ToolResult};


use super::logger::RunLogger;
use super::models::{Run, RunStatus, RunStep};

use crate::llm::prompts::SYSTEM_PROMPT;

const MAX_STEPS: usize = 50;
const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";
/// Max characters for the agent's context block (working memory)
const MAX_CONTEXT_CHARS: usize = 16_000;
/// Number of recent messages to keep after system + initial prompt
const MAX_RECENT_MESSAGES: usize = 8;
/// Max characters for a single tool response stored in history
const MAX_TOOL_RESPONSE_CHARS: usize = 4_000;

/// Configuration for a run execution
pub struct ExecutorConfig {
    pub model: String,
    pub api_key: Option<String>,
    pub max_steps: usize,
    pub headless: bool,
    pub provider: Option<String>,
    /// Minimum delay between LLM calls in milliseconds (rate limiting)
    pub min_llm_delay_ms: u64,
    /// Whether to capture screenshots after each step (disable for faster execution)
    pub capture_screenshots: bool,
    /// Allowed directories for real filesystem access
    pub allowed_directories: Vec<std::path::PathBuf>,
    /// Spawner for child agent runs (orchestration)
    pub run_spawner: Option<Arc<dyn RunSpawner>>,
    /// Current agent nesting depth (0 = top-level)
    pub agent_depth: u32,
    /// Command execution timeout in seconds (default 30)
    pub command_timeout_secs: u64,
}

/// Default minimum delay between LLM calls (2 seconds for rate limit safety)
const DEFAULT_MIN_LLM_DELAY_MS: u64 = 2000;

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            api_key: None,
            max_steps: MAX_STEPS,
            headless: false,
            provider: None,
            min_llm_delay_ms: DEFAULT_MIN_LLM_DELAY_MS,
            capture_screenshots: true,
            allowed_directories: Vec::new(),
            run_spawner: None,
            agent_depth: 0,
            command_timeout_secs: 30,
        }
    }
}

/// Run executor - manages the AI agent loop
pub struct RunExecutor {
    config: ExecutorConfig,
    registry: ToolRegistry,
    logger: RunLogger,
    browser: Arc<BrowserManager>,
    /// Cancellation token for graceful shutdown
    cancel_token: CancellationToken,
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

        // Create in-memory storage for memories/notes
        let memories = Arc::new(RwLock::new(Vec::new()));

        // Create terminal session for this run
        let terminal_session = Arc::new(tokio::sync::Mutex::new(TerminalSession::new()));

        // Create tool context
        let ctx = ToolContext {
            run_id: run_id.clone(),
            workflow_id: run.workflow_id.clone(),
            browser: Arc::clone(&self.browser),
            selector_map: Arc::clone(&selector_map),
            memories: Arc::clone(&memories),
            terminal_session: Arc::clone(&terminal_session),
            allowed_directories: self.config.allowed_directories.clone(),
            run_spawner: self.config.run_spawner.clone(),
            agent_depth: self.config.agent_depth,
        };

        // Create genai client
        // Pass API key directly through AuthResolver instead of using environment variables
        let client = if let Some(api_key) = &self.config.api_key {
            let api_key = api_key.clone();
            let auth_resolver = AuthResolver::from_resolver_fn(
                move |_model_iden: ModelIden| -> std::result::Result<Option<AuthData>, genai::resolver::Error> {
                    Ok(Some(AuthData::from_single(api_key.clone())))
                }
            );
            Client::builder().with_auth_resolver(auth_resolver).build()
        } else {
            Client::default()
        };

        // Convert our tools to genai tools
        let tools = self.build_genai_tools();

        // Keep conversation history WITHOUT screenshots (text only) to save tokens
        // Only the CURRENT page state gets a screenshot
        let mut history: Vec<ChatMessage> = vec![ChatMessage::system(SYSTEM_PROMPT)];

        // Add initial user message (text only for history)
        history.push(ChatMessage::user(user_prompt.clone()));

        let mut step_number = 0;
        let mut first_iteration = true;
        let mut last_llm_call: Option<Instant> = None;
        let mut context_block: Option<String> = None;

        // Get max_steps from run metadata (workflow override) or use config default
        let max_steps = run
            .metadata
            .get("max_steps")
            .and_then(|v| v.as_i64())
            .map(|v| v as usize)
            .unwrap_or(self.config.max_steps);

        // Agent loop
        loop {
            // Check for cancellation at the start of each iteration
            if self.cancel_token.is_cancelled() {
                self.logger.info(run_id, "Run cancelled by user");
                self.logger.status(run_id, RunStatus::Cancelled, Some("Cancelled by user".to_string()));
                return Ok(());
            }

            if step_number >= max_steps {
                self.logger.warn(run_id, format!("Reached maximum steps limit ({})", max_steps));
                self.logger.status(
                    run_id,
                    RunStatus::Completed,
                    Some("Completed (reached step limit)".to_string()),
                );
                break;
            }

            // Build fresh request: history (text-only) + current page state (WITH screenshot)
            // This way only the LATEST screenshot is sent, not all historical ones
            let (_page_state_text, chat_req) = if first_iteration {
                // First iteration: use initial prompt + screenshot
                // But ALSO populate selector_map for tools to use
                let dom_result = self.browser.get_indexed_elements().await.unwrap_or_default();
                *selector_map.write().await = dom_result.selector_map.clone();

                let screenshot = self.browser.screenshot().await.ok();
                let req = self.build_request_with_screenshot(&history, &user_prompt, screenshot, &tools);
                (user_prompt.clone(), req)
            } else {
                // Subsequent iterations: get current page state + screenshot + memories + context
                let (text, req) = self.build_current_state_request(&history, &selector_map, &memories, &tools, step_number, max_steps, context_block.as_deref()).await;
                (text, req)
            };
            first_iteration = false;

            // Log what we're sending to LLM
            let step_log = format!(
                "Step {}/{} | Sending {} messages to LLM",
                step_number + 1, max_steps, history.len() + 1 // +1 for current page state
            );
            tracing::info!("{}", step_log);
            self.logger.info(run_id, step_log);

            // Rate limiting: ensure minimum delay between LLM calls
            if let Some(last_call) = last_llm_call {
                let elapsed = last_call.elapsed();
                let min_delay = Duration::from_millis(self.config.min_llm_delay_ms);
                if elapsed < min_delay {
                    let sleep_time = min_delay - elapsed;
                    self.logger.debug(run_id, format!("Rate limiting: sleeping {}ms", sleep_time.as_millis()));
                    tokio::time::sleep(sleep_time).await;
                }
            }
            last_llm_call = Some(Instant::now());

            // Call the model with exponential backoff retry for rate limits
            const MAX_RETRIES: u32 = 5;
            const INITIAL_BACKOFF_MS: u64 = 2000; // Start with 2 seconds

            let llm_response: genai::chat::ChatResponse = 'retry: {
                let mut retry_count = 0u32;
                let mut backoff_ms = INITIAL_BACKOFF_MS;

                loop {
                    // 120s timeout for slow providers (e.g., Novita/Qwen VL)
                    let result = tokio::time::timeout(
                        Duration::from_secs(120),
                        client.exec_chat(&self.config.model, chat_req.clone(), None)
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
                                self.logger.info(run_id, format!(
                                    "⚠️ Rate limited (429). Retry {}/{} after {}ms backoff...",
                                    retry_count, MAX_RETRIES, backoff_ms
                                ));
                                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                                backoff_ms *= 2; // Exponential backoff
                                last_llm_call = Some(Instant::now()); // Reset rate limit timer
                                continue;
                            }

                            // Non-retryable error or max retries exceeded
                            let error = if is_rate_limit {
                                format!("LLM request failed after {} retries (rate limited): {}", retry_count, e)
                            } else {
                                format!("LLM request failed: {}", e)
                            };
                            self.logger.error(run_id, &error);
                            self.logger.status(run_id, RunStatus::Failed, Some(error.clone()));
                            return Err(anyhow!(error));
                        }
                    }
                }
            };

            // Extract text content before consuming the response
            let text_content = llm_response.first_text().map(|s| s.to_string());

            // Extract context block from assistant's text response (working memory)
            if let Some(ref text) = text_content {
                if let Some(ctx_text) = extract_context_block(text) {
                    tracing::debug!("Updated agent context block ({} chars)", ctx_text.len());
                    self.logger.debug(run_id, format!("Agent context updated ({} chars)", ctx_text.len()));
                    context_block = Some(ctx_text);
                }
            }

            // Check for tool calls
            let tool_calls = llm_response.into_tool_calls();

            if tool_calls.is_empty() {
                // No tool calls - use text content as the result if present
                if let Some(content) = text_content {
                    self.logger.info(run_id, "LLM returned text response, completing task");
                    self.logger.result(run_id, &content);
                } else {
                    self.logger.info(run_id, "No tool calls in response, task may be complete");
                }
                self.logger.status(run_id, RunStatus::Completed, None);
                break;
            }

            // Process each tool call
            let mut tool_responses = Vec::new();
            let mut tool_response_names = Vec::new();
            let mut is_done = false;

            for tool_call in &tool_calls {
                step_number += 1;

                let tool_name = &tool_call.fn_name;
                let params: Value = tool_call.fn_arguments.clone();

                // Resolve variables in parameters before execution
                let resolved_params = resolve_variables(&params, &variables);

                // Log tool call in function(args) format
                let params_str = serde_json::to_string(&resolved_params).unwrap_or_default();
                let params_short: String = params_str.chars().take(80).collect();
                let tool_log = format!("{}({})", tool_name, params_short);
                tracing::info!("{}", tool_log);
                self.logger.info(run_id, tool_log);

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

                // Log tool result
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

                // Take screenshot after action (if browser tool and screenshots enabled)
                if self.config.capture_screenshots && is_browser_tool(tool_name) {
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
                tool_response_names.push(tool_name.clone());

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

            // Add to history: ONLY tool calls and responses (no page state - that's always fresh)
            // The current page state is always included in the current message, so we don't need old ones
            // Memory system handles important data persistence

            // 1. The assistant's response - include both text (with context) and tool calls
            let genai_tool_calls: Vec<genai::chat::ToolCall> = tool_calls
                .iter()
                .map(|tc| genai::chat::ToolCall {
                    call_id: tc.call_id.clone(),
                    fn_name: tc.fn_name.clone(),
                    fn_arguments: tc.fn_arguments.clone(),
                })
                .collect();

            // Build assistant message with text + tool calls so context is preserved in history
            let mut assistant_parts: Vec<ContentPart> = Vec::new();
            if let Some(ref text) = text_content {
                assistant_parts.push(ContentPart::from_text(text));
            }
            for tc in &genai_tool_calls {
                assistant_parts.push(ContentPart::from(tc.clone()));
            }
            let assistant_content = MessageContent::from_parts(assistant_parts);
            history.push(ChatMessage::assistant(assistant_content));

            // 2. The tool responses (sanitized to remove large binary data from history)
            for (i, response) in tool_responses.into_iter().enumerate() {
                let tool_name = tool_response_names.get(i).map(|s| s.as_str()).unwrap_or("");
                let sanitized_content = sanitize_tool_response_for_history(&response.content, tool_name);
                history.push(ChatMessage::from(ToolResponse::new(
                    response.call_id,
                    sanitized_content,
                )));
            }

            // Sliding window: keep system prompt + initial user prompt + last few turns
            // Context block captures important state from trimmed history, so we can keep a tight window
            let max_with_prefix = 2 + MAX_RECENT_MESSAGES; // 2 initial (system + user) + recent
            if history.len() > max_with_prefix {
                let to_remove = history.len() - max_with_prefix;
                history.drain(2..2 + to_remove);
                tracing::debug!("Trimmed {} old messages from history (context block preserves state)", to_remove);
            }
        }

        // Clean up terminal session (kill any background processes)
        {
            let mut session = terminal_session.lock().await;
            if let Some(ref mut child) = session.background_child {
                let _ = child.kill().await;
            }
            session.background_child = None;
            session.background_running = false;
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

        // Build current message with optional screenshot
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

    /// Build request with current page state (text + screenshot + memories + context)
    async fn build_current_state_request(
        &self,
        history: &[ChatMessage],
        selector_map: &Arc<RwLock<SelectorMap>>,
        memories: &Arc<RwLock<Vec<crate::tools::Memory>>>,
        tools: &[Tool],
        step_number: usize,
        max_steps: usize,
        previous_context: Option<&str>,
    ) -> (String, ChatRequest) {
        let url = self.browser.current_url().await.unwrap_or_default();
        let title = self.browser.get_title().await.unwrap_or_default();

        // Get DOM extraction result from page
        let dom_result = self.browser.get_indexed_elements().await.unwrap_or_default();

        // Update the shared selector map for tools
        *selector_map.write().await = dom_result.selector_map.clone();

        // Get current memories snapshot
        let memories_snapshot = memories.read().await;

        // Build text content with context, memories, allowed directories, and step info
        let text = UserMessageBuilder::new()
            .with_previous_context(previous_context)
            .with_memories(&memories_snapshot)
            .with_allowed_directories(&self.config.allowed_directories)
            .with_browser_state(&url, &title, &dom_result)
            .with_step_info(step_number, max_steps)
            .build();

        // Take screenshot
        let screenshot = self.browser.screenshot().await.ok();

        // Build request
        let req = self.build_request_with_screenshot(history, &text, screenshot, tools);

        (text, req)
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

/// Sanitize a tool response for history storage.
/// Strips base64 image data and truncates oversized responses to save tokens.
fn sanitize_tool_response_for_history(response_json: &str, tool_name: &str) -> String {
    // For screenshot tool, just return a short summary — the screenshot is already
    // captured separately and the agent gets a fresh one each turn
    if tool_name == "screenshot" {
        return r#"{"success":true,"content":"Screenshot captured (image omitted from history)"}"#.to_string();
    }

    // For other tools, truncate if too large
    if response_json.len() <= MAX_TOOL_RESPONSE_CHARS {
        return response_json.to_string();
    }

    // Try to preserve the structure: keep first part + truncation notice
    let truncated: String = response_json.chars().take(MAX_TOOL_RESPONSE_CHARS).collect();
    format!(
        "{}... [TRUNCATED - response was {} chars, kept {}]",
        truncated,
        response_json.len(),
        MAX_TOOL_RESPONSE_CHARS
    )
}

/// Extract the `<context>...</context>` block from assistant text response.
/// Returns the inner content trimmed, capped at MAX_CONTEXT_CHARS.
fn extract_context_block(text: &str) -> Option<String> {
    let start_tag = "<context>";
    let end_tag = "</context>";

    let start = text.find(start_tag)?;
    let content_start = start + start_tag.len();
    let end = text[content_start..].find(end_tag)?;
    let content = text[content_start..content_start + end].trim();

    if content.is_empty() {
        return None;
    }

    if content.len() > MAX_CONTEXT_CHARS {
        Some(format!(
            "{}\n[TRUNCATED - context exceeded {} chars]",
            &content[..MAX_CONTEXT_CHARS],
            MAX_CONTEXT_CHARS
        ))
    } else {
        Some(content.to_string())
    }
}

/// Replace {{variable_name}} placeholders in JSON params with actual values.
/// This properly traverses the JSON structure instead of doing string replacement,
/// which prevents issues with special characters in variable values.
fn resolve_variables(params: &Value, variables: &HashMap<String, String>) -> Value {
    if variables.is_empty() {
        return params.clone();
    }

    resolve_variables_recursive(params, variables)
}

/// Recursively resolve variables in a JSON value
fn resolve_variables_recursive(value: &Value, variables: &HashMap<String, String>) -> Value {
    match value {
        Value::String(s) => {
            // Replace {{variable}} patterns in strings
            let mut result = s.clone();
            for (name, var_value) in variables {
                let pattern = format!("{{{{{}}}}}", name); // {{name}}
                result = result.replace(&pattern, var_value);
            }
            Value::String(result)
        }
        Value::Object(map) => {
            // Recursively process object values
            Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), resolve_variables_recursive(v, variables)))
                    .collect()
            )
        }
        Value::Array(arr) => {
            // Recursively process array elements
            Value::Array(
                arr.iter()
                    .map(|v| resolve_variables_recursive(v, variables))
                    .collect()
            )
        }
        // Numbers, bools, and null pass through unchanged
        _ => value.clone(),
    }
}
