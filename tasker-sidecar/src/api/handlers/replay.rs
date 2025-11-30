use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::agent::WorkflowAgent;
use crate::models::{SessionStatusResponse, StartReplayRequest, StartReplayResponse};

use super::super::state::{AppState, WsEvent};

/// Start a workflow replay session
/// AI agent is ALWAYS used - recorded workflow serves as hints/context
pub async fn start_replay(
    State(state): State<Arc<AppState>>,
    Json(request): Json<StartReplayRequest>,
) -> Result<Json<StartReplayResponse>, (StatusCode, String)> {
    let mut workflow = request.workflow.clone();

    // Resolve start_url from metadata or first navigate step if not set
    tracing::info!("Workflow start_url BEFORE resolve: '{}'", workflow.start_url);
    tracing::info!("Workflow metadata.start_url: {:?}", workflow.metadata.start_url);
    tracing::info!("Workflow steps count: {}", workflow.steps.len());

    workflow.resolve_start_url();

    tracing::info!("Workflow start_url AFTER resolve: '{}'", workflow.start_url);

    // AI agent is ALWAYS used - no mechanical replay
    let provider = request.llm_provider.as_deref().unwrap_or("google");
    let model = request.llm_model.as_deref().unwrap_or("gemini-3-pro-preview");

    // Load API key from local config (not passed via HTTP)
    let api_key = crate::config::get_api_key(provider);

    let agent = WorkflowAgent::from_provider(provider, model, api_key)
        .map_err(|e| {
            tracing::error!("Failed to create agent: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    let agent = Arc::new(agent);

    // Start execution - workflow steps serve as hints for the AI
    let mut result_rx = agent
        .execute(
            &workflow,
            request.task_description.clone(),
            request.variables.clone(),
            request.iterations,
            request.headless,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to start agent: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    // Get session
    let session = agent.session().await.ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to get session".to_string(),
    ))?;

    let session_id = session.id.clone();

    // Forward results to WebSocket
    let ws_broadcast = state.ws_broadcast.clone();
    let sid = session_id.clone();
    let agent_clone: Arc<WorkflowAgent> = Arc::clone(&agent);

    tokio::spawn(async move {
        while let Ok(result) = result_rx.recv().await {
            let _ = ws_broadcast.send(WsEvent::ReplayStep {
                session_id: sid.clone(),
                result,
            });
        }

        // Send completion event
        if let Some(final_session) = agent_clone.session().await {
            let _ = ws_broadcast.send(WsEvent::ReplayComplete {
                session_id: sid.clone(),
                session: final_session,
            });
        }
    });

    // Store active agent
    state.active_agents.insert(session_id.clone(), agent);

    tracing::info!(
        "Started AI agent session {} for workflow: {}",
        session_id,
        workflow.id
    );

    Ok(Json(StartReplayResponse {
        session_id,
        status: "running".to_string(),
    }))
}

/// Stop a replay session
pub async fn stop_replay(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let (_, agent) = state
        .active_agents
        .remove(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    agent.stop().await.map_err(|e| {
        tracing::error!("Failed to stop agent: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    tracing::info!("Stopped session {}", session_id);

    Ok(Json(serde_json::json!({ "status": "stopped" })))
}

/// Get the status of a replay session
pub async fn get_replay_status(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionStatusResponse>, (StatusCode, String)> {
    let agent = state
        .active_agents
        .get(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    let session = agent.session().await.ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to get session".to_string(),
    ))?;

    Ok(Json(SessionStatusResponse {
        session_id: session_id.clone(),
        status: session.status,
        step_count: session.total_steps,
        current_step: session.current_step,
        error: session.error,
    }))
}
