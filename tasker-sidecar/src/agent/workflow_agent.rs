use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use crate::browser::dom::{format_elements_for_llm, parse_elements, EXTRACT_ELEMENTS_SCRIPT};
use crate::browser::BrowserManager;
use crate::llm::{prompts, LLMClient, LLMConfig, LLMProvider};
use crate::models::{ReplaySession, StepResult, Viewport, Workflow};

/// Action response from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentAction {
    reasoning: String,
    action: ActionSpec,
    done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActionSpec {
    #[serde(rename = "type")]
    action_type: String,
    #[serde(default)]
    selector: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

/// AI-powered workflow agent
pub struct WorkflowAgent {
    browser: Arc<BrowserManager>,
    llm: Arc<LLMClient>,
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

        let llm = LLMClient::new(llm_config)?;

        Ok(Self {
            browser: Arc::new(BrowserManager::new()),
            llm: Arc::new(llm),
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
        let llm = Arc::clone(&self.llm);
        let session = Arc::clone(&self.session);
        let result_sender = self.result_sender.clone();
        let stopped = Arc::clone(&self.stopped);
        let max_iterations = self.max_iterations;
        let mut cancel_rx = self.cancel_sender.subscribe();

        tokio::spawn(async move {
            let mut history: Vec<String> = Vec::new();
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
                    result = agent_step(&browser, &llm, &task, &workflow, &mut history, &session) => {
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

                                history.push(format!("Error: {}", e));
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

/// Execute a single agent step
async fn agent_step(
    browser: &BrowserManager,
    llm: &LLMClient,
    task: &str,
    _workflow: &Workflow,
    history: &mut Vec<String>,
    _session: &Mutex<Option<ReplaySession>>,
) -> Result<(StepResult, bool)> {
    let start = std::time::Instant::now();

    // Get current page state
    let url = browser.current_url().await.unwrap_or_default();

    // Extract interactive elements
    let elements_json = browser.evaluate(EXTRACT_ELEMENTS_SCRIPT).await?;
    let elements = parse_elements(&elements_json)?;
    let elements_str = format_elements_for_llm(&elements);

    // Build prompt
    let user_prompt = prompts::format_page_state(&url, &elements_str, task, history);

    // Get LLM decision
    let response = llm
        .prompt_with_system(prompts::AGENT_SYSTEM_PROMPT, &user_prompt)
        .await?;

    // Strip markdown code block wrapper if present (e.g., ```json ... ```)
    let json_str = extract_json_from_response(&response);

    // Parse response
    let action: AgentAction = serde_json::from_str(json_str)
        .map_err(|e| anyhow!("Failed to parse LLM response: {}. Response: {}", e, response))?;

    tracing::info!("Agent action: {} - {}", action.action.action_type, action.reasoning);

    // Record history
    history.push(format!("{}: {}", action.action.action_type, action.reasoning));

    // Execute action
    let step_id = uuid::Uuid::new_v4().to_string();
    let result = execute_agent_action(browser, &action.action, &step_id).await;

    let duration_ms = start.elapsed().as_millis() as i32;

    match result {
        Ok(()) => Ok((StepResult::success(step_id, duration_ms), action.done)),
        Err(e) => {
            history.push(format!("Action failed: {}", e));
            Ok((StepResult::failure(step_id, e.to_string()), false))
        }
    }
}

/// Extract JSON from LLM response, stripping markdown code block wrapper if present
fn extract_json_from_response(response: &str) -> &str {
    let trimmed = response.trim();

    // Check for ```json ... ``` or ``` ... ``` wrapper
    if trimmed.starts_with("```") {
        // Find end of first line (skip ```json or ```)
        if let Some(first_newline) = trimmed.find('\n') {
            let rest = &trimmed[first_newline + 1..];
            // Find closing ```
            if let Some(end_pos) = rest.rfind("```") {
                return rest[..end_pos].trim();
            }
        }
    }

    // No wrapper, return as-is
    trimmed
}

/// Execute an action from the agent
async fn execute_agent_action(
    browser: &BrowserManager,
    action: &ActionSpec,
    _step_id: &str,
) -> Result<()> {
    match action.action_type.as_str() {
        "click" => {
            let selector = action
                .selector
                .as_ref()
                .ok_or_else(|| anyhow!("Click action requires selector"))?;
            browser.click(selector).await
        }
        "type" => {
            let selector = action
                .selector
                .as_ref()
                .ok_or_else(|| anyhow!("Type action requires selector"))?;
            let value = action
                .value
                .as_ref()
                .ok_or_else(|| anyhow!("Type action requires value"))?;
            browser.type_text(selector, value).await
        }
        "navigate" => {
            let url = action
                .url
                .as_ref()
                .ok_or_else(|| anyhow!("Navigate action requires URL"))?;
            browser.navigate(url).await
        }
        "scroll" => {
            let direction = action.value.as_deref().unwrap_or("down");
            let (x, y) = match direction {
                "up" => (0, -500),
                _ => (0, 500),
            };
            browser.scroll(x, y).await
        }
        "select" => {
            let selector = action
                .selector
                .as_ref()
                .ok_or_else(|| anyhow!("Select action requires selector"))?;
            let value = action
                .value
                .as_ref()
                .ok_or_else(|| anyhow!("Select action requires value"))?;
            browser.select(selector, value).await
        }
        "wait" => {
            let ms = action
                .value
                .as_ref()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000);
            browser.wait(ms).await
        }
        "done" => Ok(()),
        _ => Err(anyhow!("Unknown action type: {}", action.action_type)),
    }
}
