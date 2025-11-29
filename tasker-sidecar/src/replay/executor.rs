use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use crate::browser::actions::execute_with_fallback;
use crate::browser::BrowserManager;
use crate::models::{ReplaySession, StepResult, Viewport, Workflow};

/// Direct workflow executor that replays steps without AI
pub struct WorkflowExecutor {
    browser: Arc<BrowserManager>,
    session: Arc<Mutex<Option<ReplaySession>>>,
    result_sender: broadcast::Sender<StepResult>,
    cancel_sender: broadcast::Sender<()>,
    stopped: Arc<Mutex<bool>>,
}

impl WorkflowExecutor {
    pub fn new() -> Self {
        let (result_tx, _) = broadcast::channel(256);
        let (cancel_tx, _) = broadcast::channel(1);

        Self {
            browser: Arc::new(BrowserManager::new()),
            session: Arc::new(Mutex::new(None)),
            result_sender: result_tx,
            cancel_sender: cancel_tx,
            stopped: Arc::new(Mutex::new(false)),
        }
    }

    /// Execute a workflow
    pub async fn execute(
        &self,
        workflow: &Workflow,
        variables: HashMap<String, serde_json::Value>,
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

        // Launch browser and navigate to start URL
        let viewport = workflow.metadata.browser_viewport.clone().unwrap_or(Viewport {
            width: 1280,
            height: 720,
        });

        // Launch in incognito mode for clean sessions (forces fresh login)
        self.browser
            .launch_incognito(&workflow.start_url, headless, Some(viewport))
            .await?;

        // Get result receiver before spawning execution task
        let result_rx = self.result_sender.subscribe();

        // Spawn execution task
        self.spawn_executor(workflow.clone(), variables);

        Ok(result_rx)
    }

    fn spawn_executor(&self, workflow: Workflow, _variables: HashMap<String, serde_json::Value>) {
        let browser = Arc::clone(&self.browser);
        let session = Arc::clone(&self.session);
        let result_sender = self.result_sender.clone();
        let stopped = Arc::clone(&self.stopped);
        let mut cancel_rx = self.cancel_sender.subscribe();

        tokio::spawn(async move {
            tracing::info!("Starting replay with {} steps", workflow.steps.len());

            for (idx, step) in workflow.steps.iter().enumerate() {
                tracing::info!("Processing step {}/{}: {} (action: {:?})",
                    idx + 1, workflow.steps.len(), step.name, step.action.action_type);

                // Check if stopped
                if *stopped.lock().await {
                    tracing::info!("Replay stopped by user");
                    break;
                }

                // Check for cancellation
                tokio::select! {
                    biased;
                    _ = cancel_rx.recv() => {
                        tracing::info!("Replay cancelled");
                        break;
                    }
                    result = execute_step(&browser, step, &session) => {
                        match result {
                            Ok(step_result) => {
                                // Update session
                                {
                                    let mut session_guard = session.lock().await;
                                    if let Some(ref mut sess) = *session_guard {
                                        sess.current_step += 1;
                                        sess.results.push(step_result.clone());
                                    }
                                }

                                // Broadcast result
                                let _ = result_sender.send(step_result.clone());

                                // If step failed, decide whether to continue or stop
                                if !step_result.success {
                                    tracing::warn!("Step {} failed: {:?}", step.id, step_result.error);
                                    // Continue to next step (could make this configurable)
                                }

                                // Wait between steps
                                tokio::time::sleep(tokio::time::Duration::from_millis(
                                    step.wait_after_ms as u64,
                                ))
                                .await;
                            }
                            Err(e) => {
                                tracing::error!("Step execution error: {}", e);
                                let error_result = StepResult::failure(step.id.clone(), e.to_string());
                                let _ = result_sender.send(error_result);
                                break;
                            }
                        }
                    }
                }
            }

            // Mark session as completed
            {
                let mut session_guard = session.lock().await;
                if let Some(ref mut sess) = *session_guard {
                    sess.complete();
                }
            }

            // Close browser
            let _ = browser.close().await;

            tracing::info!("Replay execution completed");
        });
    }

    /// Stop the replay
    pub async fn stop(&self) -> Result<()> {
        *self.stopped.lock().await = true;
        let _ = self.cancel_sender.send(());

        let mut session_guard = self.session.lock().await;
        if let Some(ref mut session) = *session_guard {
            session.status = "stopped".to_string();
        }

        // Close browser
        self.browser.close().await?;

        tracing::info!("Replay stopped");
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

impl Default for WorkflowExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute a single workflow step
async fn execute_step(
    browser: &BrowserManager,
    step: &crate::models::WorkflowStep,
    _session: &Mutex<Option<ReplaySession>>,
) -> Result<StepResult> {
    tracing::info!("Executing step: {} - {}", step.order, step.name);

    // Use fallback-enabled execution
    let result = execute_with_fallback(browser, &step.action, &step.id).await?;

    Ok(result)
}
