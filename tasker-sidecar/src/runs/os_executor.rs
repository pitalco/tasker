//! OS-level automation executor using vision-based grid system

use anyhow::{anyhow, Result};
use genai::chat::{ChatMessage, ChatRequest, ContentPart, Tool, ToolResponse};
use genai::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::agent::UserMessageBuilder;
use crate::browser::{BrowserManager, SelectorMap};
use crate::desktop::DesktopManager;
use crate::llm::{get_auth_token, RailwayChatRequest, RailwayChatResponse, RailwayClient, RailwayMessage};
use crate::tools::{register_os_tools, ToolContext, ToolRegistry, ToolResult};

use super::executor::{ExecutorConfig, UnifiedToolCall};
use super::logger::RunLogger;
use super::models::{Run, RunStatus, RunStep};

use crate::llm::prompts::OS_SYSTEM_PROMPT;

/// OS Run executor - manages the AI agent loop for desktop automation
pub struct OsRunExecutor {
    config: ExecutorConfig,
    registry: ToolRegistry,
    logger: RunLogger,
    desktop: Arc<Mutex<DesktopManager>>,
    /// Dummy browser manager (needed for ToolContext, but not used in OS mode)
    browser: Arc<BrowserManager>,
}

impl OsRunExecutor {
    pub async fn new(
        logger: RunLogger,
        config: ExecutorConfig,
    ) -> Result<Self> {
        // Create desktop manager
        let desktop = Arc::new(Mutex::new(DesktopManager::new()?));

        // Create tool registry with OS tools
        let mut registry = ToolRegistry::new();
        register_os_tools(&mut registry, Arc::clone(&desktop));

        // Register the done tool from browser_tools (needed for task completion)
        use crate::tools::browser_tools::DoneTool;
        registry.register(Arc::new(DoneTool));

        // Create a dummy browser manager (required for ToolContext but not used)
        // This is a workaround - in a cleaner design, ToolContext would be mode-specific
        let browser = Arc::new(BrowserManager::new());

        Ok(Self {
            config,
            registry,
            logger,
            desktop,
            browser,
        })
    }

    /// Execute an OS automation run
    pub async fn execute(&self, run: &Run) -> Result<()> {
        let run_id = &run.id;

        // Update status to running
        self.logger.status(run_id, RunStatus::Running, None);
        self.logger.info(run_id, "Starting OS automation run");

        // Build the initial prompt
        let mut user_prompt = format!(
            "Task: {}",
            run.task_description
                .as_deref()
                .unwrap_or("Complete the task on the desktop")
        );

        // Add custom instructions if provided
        if let Some(instructions) = &run.custom_instructions {
            user_prompt.push_str(&format!("\n\nAdditional instructions:\n{}", instructions));
        }

        // Create selector map (unused in OS mode but required for ToolContext)
        let selector_map = Arc::new(RwLock::new(SelectorMap::new()));

        // Create tool context
        let ctx = ToolContext {
            run_id: run_id.clone(),
            workflow_id: run.workflow_id.clone(),
            browser: Arc::clone(&self.browser),
            selector_map: Arc::clone(&selector_map),
            file_repository: Some(Arc::new(self.logger.repository().clone())),
            desktop: Some(Arc::clone(&self.desktop)),
        };

        // Check if using Tasker Fast (Railway proxy)
        let uses_railway = self.config.uses_tasker_fast();
        let railway_client = if uses_railway {
            let auth_token = get_auth_token()
                .map_err(|e| {
                    let error = format!("Failed to get auth token: {}", e);
                    self.logger.error(run_id, &error);
                    anyhow!(error)
                })?
                .ok_or_else(|| {
                    let error = "Not authenticated. Please sign in to use Tasker Fast.";
                    self.logger.error(run_id, error);
                    self.logger.status(run_id, RunStatus::Failed, Some(error.to_string()));
                    anyhow!(error)
                })?;

            self.logger.info(run_id, "Using Tasker Fast (Railway proxy)");
            Some(RailwayClient::with_token(auth_token))
        } else {
            if let Some(ref api_key) = self.config.api_key {
                unsafe { std::env::set_var("ANTHROPIC_API_KEY", api_key) };
            }
            None
        };

        let client = Client::default();
        let tools = self.build_genai_tools();

        // Build initial chat request with OS system prompt
        let mut chat_req = ChatRequest::new(vec![ChatMessage::system(OS_SYSTEM_PROMPT)])
            .with_tools(tools.clone());

        // Take initial screenshot and build initial message
        let initial_message = self.build_initial_message(&user_prompt).await?;
        chat_req = chat_req.append_message(initial_message);

        let mut step_number = 0;
        let mut first_iteration = true;

        let max_steps = run
            .metadata
            .get("max_steps")
            .and_then(|v| v.as_i64())
            .map(|v| v as usize)
            .unwrap_or(self.config.max_steps);

        // Agent loop
        loop {
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

            // For subsequent iterations, take fresh screenshot
            if !first_iteration {
                let page_state = self.build_os_state_message().await?;
                chat_req = chat_req.append_message(page_state);
            }
            first_iteration = false;

            self.logger
                .debug(run_id, format!("Step {}: Calling LLM", step_number + 1));

            // Call the model
            let llm_response = if let Some(ref railway) = railway_client {
                match self.call_railway(railway, &chat_req, &tools).await {
                    Ok(res) => LLMResponse::Railway(res),
                    Err(e) => {
                        let error = format!("LLM request failed: {}", e);
                        self.logger.error(run_id, &error);
                        self.logger.status(run_id, RunStatus::Failed, Some(error));
                        return Err(e);
                    }
                }
            } else {
                match client
                    .exec_chat(&self.config.model, chat_req.clone(), None)
                    .await
                {
                    Ok(res) => LLMResponse::Genai(res),
                    Err(e) => {
                        let error = format!("LLM request failed: {}", e);
                        self.logger.error(run_id, &error);
                        self.logger.status(run_id, RunStatus::Failed, Some(error));
                        return Err(anyhow!("LLM request failed: {}", e));
                    }
                }
            };

            let tool_calls = llm_response.into_tool_calls();

            if tool_calls.is_empty() {
                self.logger
                    .info(run_id, "No tool calls in response, task may be complete");
                self.logger.status(run_id, RunStatus::Completed, None);
                break;
            }

            // Process tool calls
            let mut tool_responses = Vec::new();
            let mut is_done = false;

            for tool_call in &tool_calls {
                step_number += 1;

                let tool_name = &tool_call.fn_name;
                let params = tool_call.fn_arguments.clone();

                self.logger.info(
                    run_id,
                    format!("Executing tool: {} with params: {}", tool_name, params),
                );

                let mut step = RunStep::new(
                    run_id.clone(),
                    step_number as i32,
                    tool_name.clone(),
                    params.clone(),
                );

                self.logger.step(&step);

                let start = std::time::Instant::now();

                let result = match self.registry.execute(tool_name, params, &ctx).await {
                    Ok(r) => r,
                    Err(e) => ToolResult::error(format!("Tool execution error: {}", e)),
                };

                let duration_ms = start.elapsed().as_millis() as i64;

                step.complete(
                    result.success,
                    result.content.clone(),
                    result.error.clone(),
                    duration_ms,
                );

                // Take screenshot after OS action
                if is_os_tool(tool_name) {
                    if let Ok(screenshot) = self.take_screenshot_base64().await {
                        step.screenshot = Some(screenshot);
                    }
                }

                self.logger.update_step(&step);

                if result.success {
                    self.logger
                        .debug(run_id, format!("Tool {} succeeded: {:?}", tool_name, result.content));
                } else {
                    self.logger
                        .warn(run_id, format!("Tool {} failed: {:?}", tool_name, result.error));
                }

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

            // Append to chat history
            let genai_tool_calls = to_genai_tool_calls(&tool_calls);
            chat_req = chat_req.append_message(genai_tool_calls);
            for response in tool_responses {
                chat_req = chat_req.append_message(response);
            }
        }

        Ok(())
    }

    /// Build initial message with task and screenshot
    async fn build_initial_message(&self, task: &str) -> Result<ChatMessage> {
        let mut parts = vec![ContentPart::from_text(task)];

        // Take screenshot with grid
        let (screenshot_base64, grid_description) = {
            let mut desktop = self.desktop.lock().await;
            desktop.capture_with_grid_base64()?
        };

        // Add grid description
        let state_text = UserMessageBuilder::new_os()
            .with_os_state(&grid_description, None)
            .build();
        parts.push(ContentPart::from_text(state_text));

        // Add screenshot
        parts.push(ContentPart::from_binary_base64(
            "image/png",
            screenshot_base64,
            Some("screenshot.png".to_string()),
        ));

        Ok(ChatMessage::user(parts))
    }

    /// Build OS state message with fresh screenshot
    async fn build_os_state_message(&self) -> Result<ChatMessage> {
        let mut parts = Vec::new();

        // Take screenshot with grid
        let (screenshot_base64, grid_description) = {
            let mut desktop = self.desktop.lock().await;
            desktop.capture_with_grid_base64()?
        };

        // Get active windows
        let windows_text = {
            let desktop = self.desktop.lock().await;
            match desktop.list_windows() {
                Ok(windows) => {
                    let lines: Vec<String> = windows
                        .iter()
                        .take(10) // Limit to 10 windows
                        .map(|w| format!("- {} ({})", w.title, w.app_name))
                        .collect();
                    Some(lines.join("\n"))
                }
                Err(_) => None,
            }
        };

        let state_text = UserMessageBuilder::new_os()
            .with_os_state(&grid_description, windows_text.as_deref())
            .build();
        parts.push(ContentPart::from_text(state_text));

        parts.push(ContentPart::from_binary_base64(
            "image/png",
            screenshot_base64,
            Some("screenshot.png".to_string()),
        ));

        Ok(ChatMessage::user(parts))
    }

    /// Take a screenshot and return as base64
    async fn take_screenshot_base64(&self) -> Result<String> {
        let desktop = self.desktop.lock().await;
        desktop.capture_screen_base64()
    }

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

    async fn call_railway(
        &self,
        railway: &RailwayClient,
        chat_req: &ChatRequest,
        tools: &[Tool],
    ) -> Result<RailwayChatResponse> {
        // Convert genai messages to Railway format
        let messages: Vec<RailwayMessage> = chat_req
            .messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    genai::chat::ChatRole::System => "system",
                    genai::chat::ChatRole::User => "user",
                    genai::chat::ChatRole::Assistant => "assistant",
                    genai::chat::ChatRole::Tool => "tool",
                };

                let content = if msg.content.parts().len() == 1 {
                    if let Some(text) = msg.content.parts()[0].as_text() {
                        Value::String(text.to_string())
                    } else {
                        let parts: Vec<Value> = msg
                            .content
                            .parts()
                            .iter()
                            .map(|p| {
                                if let Some(text) = p.as_text() {
                                    json!({"type": "text", "text": text})
                                } else if let Some(binary) = p.as_binary() {
                                    let base64_str = match &binary.source {
                                        genai::chat::BinarySource::Base64(b64) => b64.to_string(),
                                        genai::chat::BinarySource::Url(url) => url.clone(),
                                    };
                                    json!({
                                        "type": "image_url",
                                        "image_url": {
                                            "url": format!("data:{};base64,{}", binary.content_type, base64_str)
                                        }
                                    })
                                } else {
                                    json!({"type": "text", "text": ""})
                                }
                            })
                            .collect();
                        Value::Array(parts)
                    }
                } else {
                    let parts: Vec<Value> = msg
                        .content
                        .parts()
                        .iter()
                        .map(|p| {
                            if let Some(text) = p.as_text() {
                                json!({"type": "text", "text": text})
                            } else if let Some(binary) = p.as_binary() {
                                let base64_str = match &binary.source {
                                    genai::chat::BinarySource::Base64(b64) => b64.to_string(),
                                    genai::chat::BinarySource::Url(url) => url.clone(),
                                };
                                json!({
                                    "type": "image_url",
                                    "image_url": {
                                        "url": format!("data:{};base64,{}", binary.content_type, base64_str)
                                    }
                                })
                            } else {
                                json!({"type": "text", "text": ""})
                            }
                        })
                        .collect();
                    Value::Array(parts)
                };

                RailwayMessage {
                    role: role.to_string(),
                    content,
                    tool_calls: None,
                    tool_call_id: None,
                }
            })
            .collect();

        let tools_json: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.schema
                    }
                })
            })
            .collect();

        let request = RailwayChatRequest {
            messages,
            max_tokens: Some(4096),
            temperature: Some(0.7),
            tools: if tools_json.is_empty() {
                None
            } else {
                Some(tools_json)
            },
            tool_choice: if tools.is_empty() {
                None
            } else {
                Some(json!("auto"))
            },
        };

        railway.chat(request).await
    }
}

/// Check if a tool is an OS tool (for screenshot capture)
fn is_os_tool(name: &str) -> bool {
    name.starts_with("os_") || name == "launch_app" || name == "list_windows"
}

/// Response wrapper for unified handling
enum LLMResponse {
    Genai(genai::chat::ChatResponse),
    Railway(RailwayChatResponse),
}

impl LLMResponse {
    fn into_tool_calls(self) -> Vec<UnifiedToolCall> {
        match self {
            LLMResponse::Genai(res) => res
                .into_tool_calls()
                .into_iter()
                .map(|tc| UnifiedToolCall {
                    call_id: tc.call_id,
                    fn_name: tc.fn_name,
                    fn_arguments: tc.fn_arguments,
                })
                .collect(),
            LLMResponse::Railway(res) => extract_railway_tool_calls(&res),
        }
    }
}

fn extract_railway_tool_calls(response: &RailwayChatResponse) -> Vec<UnifiedToolCall> {
    let mut tool_calls = Vec::new();

    if let Some(choice) = response.choices.first() {
        if let Some(calls) = &choice.message.tool_calls {
            for call in calls {
                if let Some(id) = call.get("id").and_then(|v| v.as_str()) {
                    if let Some(func) = call.get("function") {
                        let name = func
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        let arguments = func
                            .get("arguments")
                            .and_then(|v| v.as_str())
                            .and_then(|s| serde_json::from_str(s).ok())
                            .unwrap_or(Value::Object(serde_json::Map::new()));

                        tool_calls.push(UnifiedToolCall {
                            call_id: id.to_string(),
                            fn_name: name,
                            fn_arguments: arguments,
                        });
                    }
                }
            }
        }
    }

    tool_calls
}

fn to_genai_tool_calls(tool_calls: &[UnifiedToolCall]) -> Vec<genai::chat::ToolCall> {
    tool_calls
        .iter()
        .map(|tc| genai::chat::ToolCall {
            call_id: tc.call_id.clone(),
            fn_name: tc.fn_name.clone(),
            fn_arguments: tc.fn_arguments.clone(),
        })
        .collect()
}
