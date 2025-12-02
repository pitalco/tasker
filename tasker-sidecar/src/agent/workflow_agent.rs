use anyhow::{anyhow, Result};
use genai::chat::{ChatMessage, ChatRequest, ContentPart, Tool, ToolResponse};
use genai::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};

use crate::agent::UserMessageBuilder;
use crate::browser::{BrowserManager, SelectorMap};
use crate::llm::{LLMConfig, LLMProvider};
use crate::models::{RecordedAction, ReplaySession, StepResult, Viewport, Workflow};
use crate::tools::{register_all_tools, ToolContext, ToolRegistry, ToolResult};

/// AI-powered workflow agent using tool-based interaction
pub struct WorkflowAgent {
    browser: Arc<BrowserManager>,
    config: LLMConfig,
    session: Arc<Mutex<Option<ReplaySession>>>,
    result_sender: broadcast::Sender<StepResult>,
    cancel_sender: broadcast::Sender<()>,
    stopped: Arc<Mutex<bool>>,
    max_iterations: usize,
}

impl WorkflowAgent {
    /// Create a new workflow agent
    pub fn new(llm_config: LLMConfig) -> Result<Self> {
        let (result_tx, _) = broadcast::channel(256);
        let (cancel_tx, _) = broadcast::channel(1);

        Ok(Self {
            browser: Arc::new(BrowserManager::new()),
            config: llm_config,
            session: Arc::new(Mutex::new(None)),
            result_sender: result_tx,
            cancel_sender: cancel_tx,
            stopped: Arc::new(Mutex::new(false)),
            max_iterations: 50, // Safety limit
        })
    }

    /// Create from provider and model strings
    pub fn from_provider(
        provider: &str,
        model: &str,
        api_key: Option<String>,
    ) -> Result<Self> {
        let provider: LLMProvider = provider.parse()?;
        let mut config = LLMConfig::new(provider, model.to_string());
        if let Some(key) = api_key {
            config = config.with_api_key(key);
        }
        Self::new(config)
    }

    /// Execute a workflow with AI assistance
    pub async fn execute(
        &self,
        workflow: &Workflow,
        task_description: Option<String>,
        variables: HashMap<String, serde_json::Value>,
        _iterations: i32,
        headless: bool,
    ) -> Result<broadcast::Receiver<StepResult>> {
        // Create session
        let mut session = ReplaySession::new(
            workflow.id.clone(),
            workflow.steps.len() as i32,
            variables.clone(),
        );
        session.start();

        *self.session.lock().await = Some(session);
        *self.stopped.lock().await = false;

        // Launch browser
        let viewport = workflow.metadata.browser_viewport.clone().unwrap_or(Viewport {
            width: 1280,
            height: 720,
        });

        // Launch in incognito mode for clean sessions (forces fresh login)
        self.browser
            .launch_incognito(&workflow.start_url, headless, Some(viewport))
            .await?;

        // Get result receiver
        let result_rx = self.result_sender.subscribe();

        // Build task description
        let task = task_description.unwrap_or_else(|| {
            if workflow.steps.is_empty() {
                "Complete the task on this page.".to_string()
            } else {
                format!(
                    "Replay the workflow: {}. Steps: {}",
                    workflow.name,
                    workflow
                        .steps
                        .iter()
                        .map(|s| s.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        });

        // Spawn agent loop
        self.spawn_agent_loop(workflow.clone(), task);

        Ok(result_rx)
    }

    fn spawn_agent_loop(&self, workflow: Workflow, task: String) {
        let browser = Arc::clone(&self.browser);
        let config = self.config.clone();
        let session = Arc::clone(&self.session);
        let result_sender = self.result_sender.clone();
        let stopped = Arc::clone(&self.stopped);
        let max_iterations = self.max_iterations;
        let mut cancel_rx = self.cancel_sender.subscribe();

        tokio::spawn(async move {
            // Set up API key if provided
            if let Some(ref api_key) = config.api_key {
                std::env::set_var(config.provider.api_key_env_var(), api_key);
            }

            // Create genai client
            let client = Client::default();
            let model_id = config.provider.model_id(&config.model);

            // Create tool registry
            let mut registry = ToolRegistry::new();
            register_all_tools(&mut registry);

            // Convert workflow steps to RecordedAction format for hints
            let recorded_actions: Vec<RecordedAction> = workflow
                .steps
                .iter()
                .map(|step| step.into())
                .collect();

            // Create selector map storage (will be updated before each LLM call)
            let selector_map = Arc::new(RwLock::new(SelectorMap::new()));

            // Create tool context
            let ctx = ToolContext {
                run_id: workflow.id.clone(),
                browser: Arc::clone(&browser),
                selector_map: Arc::clone(&selector_map),
            };

            // Convert tools to genai format
            let tools: Vec<_> = registry
                .definitions()
                .into_iter()
                .map(|def| {
                    Tool::new(&def.name)
                        .with_description(&def.description)
                        .with_schema(def.parameters)
                })
                .collect();

            // Build initial user message
            let initial_message = build_initial_message(&browser, &task, &recorded_actions, &selector_map).await;

            // Build initial chat request with system prompt
            use crate::llm::prompts::SYSTEM_PROMPT;
            let mut chat_req = ChatRequest::new(vec![ChatMessage::system(SYSTEM_PROMPT)])
                .with_tools(tools)
                .append_message(initial_message);

            let mut iteration = 0;
            let mut consecutive_errors = 0;

            loop {
                // Check if stopped
                if *stopped.lock().await {
                    tracing::info!("Agent stopped by user");
                    break;
                }

                // Check iteration limit
                iteration += 1;
                if iteration > max_iterations {
                    tracing::warn!("Agent reached max iterations ({})", max_iterations);
                    // Send error to user
                    let error_result = StepResult::failure(
                        "max_iterations".to_string(),
                        "Agent reached maximum iterations without completing task".to_string()
                    );
                    let _ = result_sender.send(error_result);

                    let mut session_guard = session.lock().await;
                    if let Some(ref mut sess) = *session_guard {
                        sess.error = Some("Max iterations reached".to_string());
                        sess.status = "failed".to_string();
                    }
                    break;
                }

                // Check for cancellation
                tokio::select! {
                    biased;
                    _ = cancel_rx.recv() => {
                        tracing::info!("Agent cancelled");
                        break;
                    }
                    result = agent_step(
                        &browser,
                        &client,
                        &model_id,
                        &registry,
                        &ctx,
                        &selector_map,
                        &mut chat_req,
                        &session,
                        &result_sender,
                    ) => {
                        match result {
                            Ok((step_result, done)) => {
                                consecutive_errors = 0; // Reset on success

                                // Update session
                                {
                                    let mut session_guard = session.lock().await;
                                    if let Some(ref mut sess) = *session_guard {
                                        sess.current_step += 1;
                                        sess.results.push(step_result.clone());
                                    }
                                }

                                // Broadcast result
                                let _ = result_sender.send(step_result);

                                // Check if done
                                if done {
                                    tracing::info!("Agent completed task");
                                    break;
                                }

                                // Wait between steps
                                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            }
                            Err(e) => {
                                let error_str = e.to_string();
                                tracing::error!("Agent step error: {}", error_str);
                                consecutive_errors += 1;

                                // Check for fatal errors that shouldn't be retried
                                let is_fatal = error_str.contains("ApiKeyEnvNotFound") ||
                                               error_str.contains("API key") ||
                                               error_str.contains("authentication") ||
                                               error_str.contains("unauthorized") ||
                                               error_str.contains("401");

                                if is_fatal {
                                    tracing::error!("Fatal authentication error - stopping agent");

                                    // Send error result to user
                                    let error_result = StepResult::failure(
                                        "auth_error".to_string(),
                                        format!("LLM authentication failed. Please check your API key in Settings. Error: {}", error_str)
                                    );
                                    let _ = result_sender.send(error_result);

                                    // Update session with error
                                    {
                                        let mut session_guard = session.lock().await;
                                        if let Some(ref mut sess) = *session_guard {
                                            sess.error = Some("Authentication failed: Please check your API key".to_string());
                                            sess.status = "failed".to_string();
                                        }
                                    }
                                    break;
                                }

                                // Too many consecutive errors - something is wrong
                                if consecutive_errors >= 3 {
                                    tracing::error!("Too many consecutive errors ({}) - stopping agent", consecutive_errors);

                                    let error_result = StepResult::failure(
                                        "repeated_errors".to_string(),
                                        format!("Agent failed after {} consecutive errors: {}", consecutive_errors, error_str)
                                    );
                                    let _ = result_sender.send(error_result);

                                    let mut session_guard = session.lock().await;
                                    if let Some(ref mut sess) = *session_guard {
                                        sess.error = Some(format!("Failed after {} errors: {}", consecutive_errors, error_str));
                                        sess.status = "failed".to_string();
                                    }
                                    break;
                                }

                                // Wait a bit before retrying
                                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                            }
                        }
                    }
                }
            }

            // Mark session as completed (only if not already failed)
            {
                let mut session_guard = session.lock().await;
                if let Some(ref mut sess) = *session_guard {
                    if sess.status != "failed" {
                        sess.complete();
                    }
                }
            }

            // Close browser
            let _ = browser.close().await;

            tracing::info!("Agent execution completed");
        });
    }

    /// Stop the agent
    pub async fn stop(&self) -> Result<()> {
        *self.stopped.lock().await = true;
        let _ = self.cancel_sender.send(());

        let mut session_guard = self.session.lock().await;
        if let Some(ref mut session) = *session_guard {
            session.status = "stopped".to_string();
        }

        self.browser.close().await?;

        tracing::info!("Agent stopped");
        Ok(())
    }

    /// Get current session
    pub async fn session(&self) -> Option<ReplaySession> {
        self.session.lock().await.clone()
    }

    /// Subscribe to step results
    pub fn subscribe_results(&self) -> broadcast::Receiver<StepResult> {
        self.result_sender.subscribe()
    }
}

/// Build initial user message with task and screenshot
async fn build_initial_message(
    browser: &BrowserManager,
    task: &str,
    recorded_actions: &[RecordedAction],
    selector_map: &Arc<RwLock<SelectorMap>>,
) -> ChatMessage {
    let url = browser.current_url().await.unwrap_or_default();
    let title = browser.get_title().await.unwrap_or_default();

    // Get DOM extraction result from page
    let dom_result = browser.get_indexed_elements().await.unwrap_or_default();

    // Update the shared selector map for tools
    *selector_map.write().await = dom_result.selector_map.clone();

    // Build user message text
    let recorded_workflow = if recorded_actions.is_empty() {
        None
    } else {
        Some(recorded_actions)
    };

    let text = UserMessageBuilder::new()
        .with_recorded_workflow(recorded_workflow)
        .with_browser_state(&url, &title, &dom_result)
        .build();

    // Add task description
    let full_text = format!("Task: {}\n\n{}", task, text);

    // Build message with screenshot
    let mut parts = vec![ContentPart::from_text(&full_text)];

    // Try to take a screenshot
    if let Ok(screenshot_base64) = browser.screenshot().await {
        parts.push(ContentPart::from_binary_base64(
            "image/png",
            screenshot_base64,
            Some("screenshot.png".to_string()),
        ));
    }

    ChatMessage::user(parts)
}

/// Execute a single agent step using tool-based interaction
async fn agent_step(
    browser: &BrowserManager,
    client: &Client,
    model_id: &str,
    registry: &ToolRegistry,
    ctx: &ToolContext,
    selector_map: &Arc<RwLock<SelectorMap>>,
    chat_req: &mut ChatRequest,
    _session: &Mutex<Option<ReplaySession>>,
    _result_sender: &broadcast::Sender<StepResult>,
) -> Result<(StepResult, bool)> {
    // Log the request being sent to the LLM
    println!("========== LLM REQUEST ==========");
    println!("Model: {}", model_id);
    for (i, msg) in chat_req.messages.iter().enumerate() {
        println!("Message[{}] role: {:?}", i, msg.role);
        for part in msg.content.parts() {
            if let ContentPart::Text(text) = part {
                println!("Message[{}] text:\n{}", i, text);
            } else {
                println!("Message[{}] [IMAGE]", i);
            }
        }
    }
    println!("=================================");

    // Call the model
    let chat_res = client
        .exec_chat(model_id, chat_req.clone(), None)
        .await
        .map_err(|e| anyhow!("LLM request failed: {}", e))?;

    // Log the response
    println!("========== LLM RESPONSE ==========");
    println!("Response: {:?}", chat_res);
    println!("==================================");

    // Check for tool calls
    let tool_calls = chat_res.into_tool_calls();

    if tool_calls.is_empty() {
        // No tool calls - task may be complete
        tracing::info!("No tool calls in response, task may be complete");
        let step_id = uuid::Uuid::new_v4().to_string();
        return Ok((StepResult::success(step_id, 0), true));
    }

    // Process each tool call
    let mut tool_responses = Vec::new();
    let mut is_done = false;
    let mut step_results = Vec::new();

    for tool_call in &tool_calls {
        let start = std::time::Instant::now();
        let tool_name = &tool_call.fn_name;
        let params: Value = tool_call.fn_arguments.clone();

        tracing::info!("Executing tool: {} with params: {}", tool_name, params);

        // Execute the tool
        let result = match registry.execute(tool_name, params, ctx).await {
            Ok(r) => r,
            Err(e) => ToolResult::error(format!("Tool execution error: {}", e)),
        };

        let duration_ms = start.elapsed().as_millis() as i32;
        let step_id = uuid::Uuid::new_v4().to_string();

        // Create step result
        let step_result = if result.success {
            StepResult::success(step_id.clone(), duration_ms)
        } else {
            StepResult::failure(step_id.clone(), result.error.clone().unwrap_or_default())
        };

        step_results.push(step_result);

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
        }
    }

    // Append tool calls and responses to chat history
    *chat_req = chat_req.clone().append_message(tool_calls);
    for response in tool_responses {
        *chat_req = chat_req.clone().append_message(response);
    }

    // Add current page state with screenshot for next iteration
    let page_state = build_page_state_message(browser, selector_map).await;
    *chat_req = chat_req.clone().append_message(page_state);

    // Return the first step result (or last if multiple)
    let final_result = step_results.pop().unwrap_or_else(|| {
        StepResult::success(uuid::Uuid::new_v4().to_string(), 0)
    });

    Ok((final_result, is_done))
}

/// Build a follow-up message with current page state
async fn build_page_state_message(
    browser: &BrowserManager,
    selector_map: &Arc<RwLock<SelectorMap>>,
) -> ChatMessage {
    let url = browser.current_url().await.unwrap_or_default();
    let title = browser.get_title().await.unwrap_or_default();

    // Get DOM extraction result from page
    let dom_result = browser.get_indexed_elements().await.unwrap_or_default();

    // Update the shared selector map for tools
    *selector_map.write().await = dom_result.selector_map.clone();

    // Use UserMessageBuilder for formatting
    let text = UserMessageBuilder::new()
        .with_browser_state(&url, &title, &dom_result)
        .build();

    // Build message with screenshot
    let mut parts = vec![ContentPart::from_text(&text)];

    // Try to take a screenshot
    if let Ok(screenshot_base64) = browser.screenshot().await {
        parts.push(ContentPart::from_binary_base64(
            "image/png",
            screenshot_base64,
            Some("screenshot.png".to_string()),
        ));
    }

    ChatMessage::user(parts)
}
