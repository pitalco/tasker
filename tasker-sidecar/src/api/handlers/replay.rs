use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

use crate::browser::BrowserManager;
use crate::models::{SessionStatusResponse, StartReplayRequest, StartReplayResponse, StepResult, Viewport};
use crate::runs::{ExecutorConfig, Run, RunEvent, RunExecutor, RunLogger, RunStatus};

use super::super::state::{AppState, WsEvent};

/// Convert RunStep to StepResult for WebSocket compatibility
fn step_to_result(step: &crate::runs::RunStep) -> StepResult {
    let params_str = step.params.to_string();
    // Truncate long params for display
    let params_display = if params_str.len() > 100 {
        format!("{}...", &params_str[..97])
    } else {
        params_str
    };

    if step.success {
        StepResult::success_with_tool(
            step.id.clone(),
            step.duration_ms as i32,
            step.tool_name.clone(),
            params_display,
        )
    } else {
        StepResult::failure_with_tool(
            step.id.clone(),
            step.error.clone().unwrap_or_default(),
            step.tool_name.clone(),
            params_display,
        )
    }
}

/// Start a workflow replay session
/// Uses RunExecutor for execution with database persistence
pub async fn start_replay(
    State(state): State<Arc<AppState>>,
    Json(request): Json<StartReplayRequest>,
) -> Result<Json<StartReplayResponse>, (StatusCode, String)> {
    tracing::info!("Starting replay with task_description: {:?}", request.task_description);

    let mut workflow = request.workflow.clone();

    // Resolve start_url from metadata or first navigate step if not set
    workflow.resolve_start_url();
    tracing::info!("Workflow start_url resolved to: '{}'", workflow.start_url);

    // Get repository for persistence
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Runs repository not initialized".to_string(),
        )
    })?;

    // Create Run from workflow
    let mut run = Run::new(
        Some(workflow.id.clone()),
        Some(workflow.name.clone()),
        request.task_description.clone(),
        None, // custom_instructions
    );

    // Add workflow steps as hints in metadata, include variables for substitution
    let hints = serde_json::to_value(&workflow.steps).unwrap_or_default();
    let variables = serde_json::to_value(&request.variables).unwrap_or_default();
    run.metadata = json!({
        "hints": hints,
        "start_url": workflow.start_url,
        "variables": variables,
        "stop_when": request.stop_when.as_deref().or(workflow.stop_when.as_deref()),
        "max_steps": request.max_steps.or(workflow.max_steps),
    });

    let run_id = run.id.clone();

    // Save to database
    repo.create_run(&run).map_err(|e| {
        tracing::error!("Failed to create run: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    // Store in active runs for tracking
    state.active_runs.insert(run_id.clone(), run.clone());

    // Get LLM config
    let provider = request.llm_provider.as_deref().unwrap_or("google");
    let model = request.llm_model.as_deref().unwrap_or("gemini-3-pro-preview");

    // Load API key from local config
    let api_key = crate::config::get_api_key(provider);

    // Set up environment variable for the provider (using thread-safe helper)
    if let Some(ref key) = api_key {
        let env_var = crate::config::get_env_var_for_provider(provider);
        crate::config::set_api_key_env(env_var, key);
    }

    // Create browser manager
    let browser = Arc::new(BrowserManager::new());

    // Get viewport from workflow metadata
    let viewport = workflow.metadata.browser_viewport.clone().unwrap_or(Viewport {
        width: 1280,
        height: 720,
    });

    // Launch browser
    browser
        .launch_incognito(&workflow.start_url, request.headless, Some(viewport))
        .await
        .map_err(|e| {
            tracing::error!("Failed to launch browser: {}", e);
            // Clean up the run record
            let _ = repo.update_run_status(&run_id, RunStatus::Failed, Some(&e.to_string()));
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    // Create logger and executor
    let logger = RunLogger::new(repo.clone());

    let config = ExecutorConfig {
        model: model.to_string(),
        api_key,
        max_steps: 50,
        headless: request.headless,
        provider: Some(provider.to_string()),
        min_llm_delay_ms: 2000, // 2 seconds minimum between LLM calls
        capture_screenshots: true, // Enable screenshots by default for debugging
    };

    let executor = RunExecutor::new(logger.clone(), Arc::clone(&browser), config);
    let cancel_token = executor.cancel_token();

    // Store cancel token for external cancellation
    state.active_executors.insert(run_id.clone(), cancel_token);

    // Subscribe to logger events and forward to WebSocket
    let mut event_rx = logger.subscribe();
    let ws_broadcast = state.ws_broadcast.clone();
    let run_id_ws = run_id.clone();

    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match event {
                RunEvent::Step { run_id: rid, step } => {
                    // Convert RunStep to StepResult for WebSocket
                    let result = step_to_result(&step);
                    let _ = ws_broadcast.send(WsEvent::ReplayStep {
                        session_id: rid,
                        result,
                    });
                }
                RunEvent::Status { run_id: rid, status, error } => {
                    if status == RunStatus::Completed || status == RunStatus::Failed {
                        // Build a minimal ReplaySession for compatibility
                        let session = crate::models::ReplaySession {
                            id: rid.clone(),
                            workflow_id: run_id_ws.clone(),
                            status: status.as_str().to_string(),
                            current_step: 0,
                            total_steps: 0,
                            results: Vec::new(),
                            variables: std::collections::HashMap::new(),
                            error,
                            started_at: None,
                            completed_at: None,
                        };
                        let _ = ws_broadcast.send(WsEvent::ReplayComplete {
                            session_id: rid,
                            session,
                        });
                    }
                }
                RunEvent::Log { .. } => {
                    // Logs are persisted to DB, no WebSocket broadcast needed
                }
            }
        }
    });

    // Execute in background
    let run_for_exec = run.clone();
    let browser_for_cleanup = Arc::clone(&browser);
    let state_for_cleanup = Arc::clone(&state);
    let run_id_for_cleanup = run_id.clone();
    let shutdown_token = state.shutdown_token.clone();

    tokio::spawn(async move {
        // Listen for both executor completion and global shutdown
        let result = tokio::select! {
            res = executor.execute(&run_for_exec) => res,
            _ = shutdown_token.cancelled() => {
                // Global shutdown - cancel the executor
                executor.cancel();
                // Give executor time to handle cancellation
                tokio::time::sleep(Duration::from_millis(100)).await;
                Err(anyhow::anyhow!("Process shutdown"))
            }
        };

        match &result {
            Ok(()) => tracing::info!("Run {} finished", run_id_for_cleanup),
            Err(e) => tracing::warn!("Run {} interrupted: {}", run_id_for_cleanup, e),
        }

        // Clean up browser
        let _ = browser_for_cleanup.close().await;

        // Remove from tracking maps
        state_for_cleanup.active_runs.remove(&run_id_for_cleanup);
        state_for_cleanup.active_executors.remove(&run_id_for_cleanup);
    });

    tracing::info!("Started run {} for workflow: {}", run_id, workflow.id);

    Ok(Json(StartReplayResponse {
        session_id: run_id,
        status: "running".to_string(),
    }))
}

/// Stop a replay session
pub async fn stop_replay(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Get repository
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Runs repository not initialized".to_string(),
        )
    })?;

    // Cancel the executor if running
    if let Some((_, token)) = state.active_executors.remove(&session_id) {
        token.cancel();
        // Give executor time to handle cancellation gracefully
        tokio::time::sleep(Duration::from_millis(100)).await;
        tracing::info!("Cancelled executor for run {}", session_id);
    }

    // Update status to cancelled in database
    repo.update_run_status(&session_id, RunStatus::Cancelled, Some("Cancelled by user"))
        .map_err(|e| {
            tracing::error!("Failed to cancel run: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    // Remove from active runs
    state.active_runs.remove(&session_id);

    tracing::info!("Stopped/cancelled run {}", session_id);

    Ok(Json(serde_json::json!({ "status": "stopped" })))
}

/// Get the status of a replay session
pub async fn get_replay_status(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionStatusResponse>, (StatusCode, String)> {
    // First check active runs
    if let Some(run) = state.active_runs.get(&session_id) {
        return Ok(Json(SessionStatusResponse {
            session_id: session_id.clone(),
            status: run.status.as_str().to_string(),
            step_count: run.steps.len() as i32,
            current_step: run.steps.len() as i32,
            error: run.error.clone(),
        }));
    }

    // Check repository for completed runs
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Runs repository not initialized".to_string(),
        )
    })?;

    let run = repo
        .get_run(&session_id)
        .map_err(|e| {
            tracing::error!("Failed to get run: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    Ok(Json(SessionStatusResponse {
        session_id: session_id.clone(),
        status: run.status.as_str().to_string(),
        step_count: run.steps.len() as i32,
        current_step: run.steps.len() as i32,
        error: run.error,
    }))
}
