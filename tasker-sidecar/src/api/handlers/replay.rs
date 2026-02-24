use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

use crate::desktop::DesktopManager;
use crate::models::{SessionStatusResponse, StartReplayRequest, StartReplayResponse, StepResult};
use crate::runs::{ExecutorConfig, Run, RunEvent, RunExecutor, RunLogger, RunStatus};

use super::super::state::{AppState, WsEvent};

/// Convert RunStep to StepResult for WebSocket compatibility
fn step_to_result(step: &crate::runs::RunStep) -> StepResult {
    let params_str = step.params.to_string();
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

/// Start a desktop automation replay session
pub async fn start_replay(
    State(state): State<Arc<AppState>>,
    Json(request): Json<StartReplayRequest>,
) -> Result<Json<StartReplayResponse>, (StatusCode, String)> {
    tracing::info!(
        "Starting desktop automation with task_description: {:?}",
        request.task_description
    );

    let workflow = request.workflow.clone();

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
        None,
    );

    // Add metadata
    let hints = serde_json::to_value(&workflow.steps).unwrap_or_default();
    let variables = serde_json::to_value(&request.variables).unwrap_or_default();
    run.metadata = json!({
        "hints": hints,
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

    // Store in active runs
    state.active_runs.insert(run_id.clone(), run.clone());

    // Get LLM config
    let provider = request.llm_provider.as_deref().unwrap_or("google");
    let model = request
        .llm_model
        .as_deref()
        .unwrap_or("gemini-3-pro-preview");

    // Load API key
    let api_key = crate::config::get_api_key(provider);

    if let Some(ref key) = api_key {
        let env_var = crate::config::get_env_var_for_provider(provider);
        crate::config::set_api_key_env(env_var, key);
    }

    // Create DesktopManager
    let desktop = Arc::new(DesktopManager::new().map_err(|e| {
        tracing::error!("Failed to create DesktopManager: {}", e);
        let _ = repo.update_run_status(&run_id, RunStatus::Failed, Some(&e.to_string()));
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?);

    // Store desktop manager for pause/resume
    state
        .desktop_managers
        .insert(run_id.clone(), Arc::clone(&desktop));

    // Create logger and executor
    let logger = RunLogger::new(repo.clone());

    let config = ExecutorConfig {
        model: model.to_string(),
        api_key,
        max_steps: 50,
        provider: Some(provider.to_string()),
        min_llm_delay_ms: 2000,
    };

    let executor = RunExecutor::new(logger.clone(), Arc::clone(&desktop), config);
    let cancel_token = executor.cancel_token();

    // Store cancel token
    state
        .active_executors
        .insert(run_id.clone(), cancel_token);

    // Subscribe to logger events and forward to WebSocket
    let mut event_rx = logger.subscribe();
    let ws_broadcast = state.ws_broadcast.clone();
    let run_id_ws = run_id.clone();

    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match event {
                RunEvent::Step { run_id: rid, step } => {
                    let result = step_to_result(&step);
                    let _ = ws_broadcast.send(WsEvent::ReplayStep {
                        session_id: rid,
                        result,
                    });
                }
                RunEvent::Status {
                    run_id: rid,
                    status,
                    error,
                } => {
                    if status == RunStatus::Completed || status == RunStatus::Failed {
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
                RunEvent::Log { .. } => {}
            }
        }
    });

    // Execute in background
    let run_for_exec = run.clone();
    let state_for_cleanup = Arc::clone(&state);
    let run_id_for_cleanup = run_id.clone();
    let shutdown_token = state.shutdown_token.clone();

    tokio::spawn(async move {
        let result = tokio::select! {
            res = executor.execute(&run_for_exec) => res,
            _ = shutdown_token.cancelled() => {
                executor.cancel();
                tokio::time::sleep(Duration::from_millis(100)).await;
                Err(anyhow::anyhow!("Process shutdown"))
            }
        };

        match &result {
            Ok(()) => tracing::info!("Run {} finished", run_id_for_cleanup),
            Err(e) => tracing::warn!("Run {} interrupted: {}", run_id_for_cleanup, e),
        }

        // Clean up
        state_for_cleanup
            .active_runs
            .remove(&run_id_for_cleanup);
        state_for_cleanup
            .active_executors
            .remove(&run_id_for_cleanup);
        state_for_cleanup
            .desktop_managers
            .remove(&run_id_for_cleanup);
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
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Runs repository not initialized".to_string(),
        )
    })?;

    // Cancel the executor
    if let Some((_, token)) = state.active_executors.remove(&session_id) {
        token.cancel();
        tokio::time::sleep(Duration::from_millis(100)).await;
        tracing::info!("Cancelled executor for run {}", session_id);
    }

    // Update status
    repo.update_run_status(&session_id, RunStatus::Cancelled, Some("Cancelled by user"))
        .map_err(|e| {
            tracing::error!("Failed to cancel run: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    // Clean up
    state.active_runs.remove(&session_id);
    state.desktop_managers.remove(&session_id);

    tracing::info!("Stopped/cancelled run {}", session_id);

    Ok(Json(json!({ "status": "stopped" })))
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

/// Pause a running automation
pub async fn pause_run(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if let Some(desktop) = state.desktop_managers.get(&run_id) {
        desktop.pause();
        tracing::info!("Paused run {}", run_id);
        Ok(Json(json!({ "status": "paused" })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            format!("Run {} not found or not running", run_id),
        ))
    }
}

/// Resume a paused automation
pub async fn resume_run(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if let Some(desktop) = state.desktop_managers.get(&run_id) {
        desktop.resume();
        tracing::info!("Resumed run {}", run_id);
        Ok(Json(json!({ "status": "running" })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            format!("Run {} not found or not running", run_id),
        ))
    }
}
