//! OS automation API handlers

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::models::StepResult;
use crate::runs::{ExecutorConfig, OsRunExecutor, Run, RunEvent, RunLogger, RunStatus};

use super::super::state::{AppState, WsEvent};

/// Request to start an OS automation session
#[derive(Debug, Deserialize)]
pub struct StartOsRequest {
    /// Task description for the agent
    pub task_description: String,
    /// Custom instructions for the agent
    pub custom_instructions: Option<String>,
    /// LLM provider (e.g., "anthropic", "openai", "tasker-fast")
    pub llm_provider: Option<String>,
    /// LLM model (e.g., "claude-sonnet-4-20250514")
    pub llm_model: Option<String>,
    /// Maximum number of steps
    pub max_steps: Option<i32>,
}

/// Response from starting an OS automation session
#[derive(Debug, Serialize)]
pub struct StartOsResponse {
    pub run_id: String,
    pub status: String,
}

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

/// Start an OS automation session
pub async fn start_os_automation(
    State(state): State<Arc<AppState>>,
    Json(request): Json<StartOsRequest>,
) -> Result<Json<StartOsResponse>, (StatusCode, String)> {
    tracing::info!("Starting OS automation with task: {}", request.task_description);

    // Get repository for persistence
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Runs repository not initialized".to_string(),
        )
    })?;

    // Create Run
    let mut run = Run::new(
        None, // no workflow_id
        None, // no workflow_name
        Some(request.task_description.clone()),
        request.custom_instructions.clone(),
    );

    // Set metadata for OS mode
    run.metadata = json!({
        "mode": "os",
        "max_steps": request.max_steps,
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
    let provider = request.llm_provider.as_deref().unwrap_or("anthropic");
    let model = request.llm_model.as_deref().unwrap_or("claude-sonnet-4-20250514");

    // Load API key from local config
    let api_key = crate::config::get_api_key(provider);

    // Set up environment variable for the provider
    if let Some(ref key) = api_key {
        let env_var = match provider {
            "anthropic" => "ANTHROPIC_API_KEY",
            "openai" => "OPENAI_API_KEY",
            "google" | "gemini" => "GEMINI_API_KEY",
            _ => "ANTHROPIC_API_KEY",
        };
        std::env::set_var(env_var, key);
    }

    // Create logger and executor config
    let logger = RunLogger::new(repo.clone());

    let config = ExecutorConfig {
        model: model.to_string(),
        api_key,
        max_steps: request.max_steps.unwrap_or(50) as usize,
        headless: false, // Not used for OS mode
        provider: Some(provider.to_string()),
    };

    // Create OS executor
    let executor = match OsRunExecutor::new(logger.clone(), config).await {
        Ok(exec) => exec,
        Err(e) => {
            tracing::error!("Failed to create OS executor: {}", e);
            let _ = repo.update_run_status(&run_id, RunStatus::Failed, Some(&e.to_string()));
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
        }
    };

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
                RunEvent::Status { run_id: rid, status, error } => {
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
                RunEvent::Log { .. } => {
                    // Logs are persisted to DB
                }
            }
        }
    });

    // Execute in background
    let run_for_exec = run.clone();
    let state_for_cleanup = Arc::clone(&state);
    let run_id_for_cleanup = run_id.clone();

    tokio::spawn(async move {
        let result = executor.execute(&run_for_exec).await;

        if let Err(e) = result {
            tracing::error!("OS run execution failed: {}", e);
        }

        // Remove from active runs
        state_for_cleanup.active_runs.remove(&run_id_for_cleanup);

        tracing::info!("OS run {} completed and cleaned up", run_id_for_cleanup);
    });

    tracing::info!("Started OS automation run {}", run_id);

    Ok(Json(StartOsResponse {
        run_id,
        status: "running".to_string(),
    }))
}

/// Stop an OS automation session
pub async fn stop_os_automation(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Runs repository not initialized".to_string(),
        )
    })?;

    repo.update_run_status(&run_id, RunStatus::Cancelled, None)
        .map_err(|e| {
            tracing::error!("Failed to cancel OS run: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    state.active_runs.remove(&run_id);

    tracing::info!("Stopped OS automation run {}", run_id);

    Ok(Json(json!({ "status": "stopped" })))
}
