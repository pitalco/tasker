use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::runs::{Run, RunListQuery, RunListResponse, RunStatus};

use super::super::state::AppState;

/// List runs with optional filters
pub async fn list_runs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RunListQuery>,
) -> Result<Json<RunListResponse>, (StatusCode, String)> {
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Runs repository not initialized".to_string(),
        )
    })?;

    let (runs, total) = repo.list_runs(&query).map_err(|e| {
        tracing::error!("Failed to list runs: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(RunListResponse {
        runs,
        total,
        page: query.page,
        per_page: query.per_page,
    }))
}

/// Get a specific run by ID
pub async fn get_run(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<Run>, (StatusCode, String)> {
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Runs repository not initialized".to_string(),
        )
    })?;

    let run = repo
        .get_run(&run_id)
        .map_err(|e| {
            tracing::error!("Failed to get run: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Run {} not found", run_id)))?;

    Ok(Json(run))
}

/// Create a new run request
#[derive(Debug, Deserialize)]
pub struct StartRunRequest {
    pub workflow_id: Option<String>,
    pub workflow_name: Option<String>,
    pub task_description: Option<String>,
    pub custom_instructions: Option<String>,
    pub start_url: Option<String>,
    #[serde(default)]
    pub headless: bool,
    #[serde(default = "default_viewport_width")]
    pub viewport_width: i32,
    #[serde(default = "default_viewport_height")]
    pub viewport_height: i32,
    pub hints: Option<serde_json::Value>,
}

fn default_viewport_width() -> i32 {
    1280
}

fn default_viewport_height() -> i32 {
    720
}

/// Start run response
#[derive(Debug, Serialize)]
pub struct StartRunResponse {
    pub run_id: String,
    pub status: String,
}

/// Start a new run (create and begin execution)
pub async fn start_run(
    State(state): State<Arc<AppState>>,
    Json(request): Json<StartRunRequest>,
) -> Result<Json<StartRunResponse>, (StatusCode, String)> {
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Runs repository not initialized".to_string(),
        )
    })?;

    // Create the run using Run::new()
    let mut run = Run::new(
        request.workflow_id,
        request.workflow_name,
        request.task_description,
        request.custom_instructions,
    );

    // Build metadata
    let mut metadata = serde_json::Map::new();
    if let Some(hints) = request.hints {
        metadata.insert("hints".to_string(), hints);
    }
    if let Some(url) = &request.start_url {
        metadata.insert("start_url".to_string(), serde_json::Value::String(url.clone()));
    }
    run.metadata = serde_json::Value::Object(metadata);

    let run_id = run.id.clone();

    // Save to database
    repo.create_run(&run).map_err(|e| {
        tracing::error!("Failed to create run: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    // Store run ID for tracking active runs
    state.active_runs.insert(run_id.clone(), run.clone());

    tracing::info!("Created run {}", run_id);

    // Note: Actual execution would be started by a separate background process
    // or by calling the executor directly. For now, we just create the run record.

    Ok(Json(StartRunResponse {
        run_id,
        status: "pending".to_string(),
    }))
}

/// Cancel a run
pub async fn cancel_run(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Runs repository not initialized".to_string(),
        )
    })?;

    // Update status to cancelled
    repo.update_run_status(&run_id, RunStatus::Cancelled, None)
        .map_err(|e| {
            tracing::error!("Failed to cancel run: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    // Remove from active runs
    state.active_runs.remove(&run_id);

    tracing::info!("Cancelled run {}", run_id);

    Ok(Json(serde_json::json!({
        "run_id": run_id,
        "status": "cancelled"
    })))
}

/// Delete a run
pub async fn delete_run(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Runs repository not initialized".to_string(),
        )
    })?;

    let deleted = repo.delete_run(&run_id).map_err(|e| {
        tracing::error!("Failed to delete run: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    if !deleted {
        return Err((StatusCode::NOT_FOUND, format!("Run {} not found", run_id)));
    }

    // Remove from active runs if present
    state.active_runs.remove(&run_id);

    tracing::info!("Deleted run {}", run_id);

    Ok(Json(serde_json::json!({
        "run_id": run_id,
        "deleted": true
    })))
}

/// Get run steps
pub async fn get_run_steps(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<Vec<crate::runs::RunStep>>, (StatusCode, String)> {
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Runs repository not initialized".to_string(),
        )
    })?;

    let steps = repo.get_steps_for_run(&run_id).map_err(|e| {
        tracing::error!("Failed to get run steps: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(steps))
}

/// Get run logs
pub async fn get_run_logs(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<Vec<crate::runs::RunLog>>, (StatusCode, String)> {
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Runs repository not initialized".to_string(),
        )
    })?;

    let logs = repo.get_logs_for_run(&run_id).map_err(|e| {
        tracing::error!("Failed to get run logs: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(logs))
}
