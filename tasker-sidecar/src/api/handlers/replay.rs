use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::agent::WorkflowAgent;
use crate::models::{ReplaySession, SessionStatusResponse, StartReplayRequest, StartReplayResponse};
use crate::replay::WorkflowExecutor;

use super::super::state::{ActiveReplay, AppState, WsEvent};

/// Start a workflow replay session
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

    if request.use_ai {
        // Use AI-powered agent
        let provider = request.llm_provider.as_deref().unwrap_or("gemini");
        let model = request.llm_model.as_deref().unwrap_or("gemini-2.5-flash");

        // Load API key from local config (not passed via HTTP)
        let api_key = crate::config::get_api_key(provider);

        let agent = WorkflowAgent::from_provider(provider, model, api_key)
            .map_err(|e| {
                tracing::error!("Failed to create agent: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            })?;

        let agent = Arc::new(agent);

        // Start execution
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
                tracing::error!("Failed to start agent replay: {}", e);
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

        // Store replay
        state.replays.insert(
            session_id.clone(),
            ActiveReplay::Agent {
                agent,
                session: session.clone(),
            },
        );

        tracing::info!(
            "Started AI replay session {} for workflow: {}",
            session_id,
            workflow.id
        );

        Ok(Json(StartReplayResponse {
            session_id,
            status: "running".to_string(),
        }))
    } else {
        // Use direct executor
        let executor = Arc::new(WorkflowExecutor::new());

        // Start execution
        let mut result_rx = executor
            .execute(&workflow, request.variables.clone(), request.headless)
            .await
            .map_err(|e| {
                tracing::error!("Failed to start direct replay: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            })?;

        // Get session
        let session = executor.session().await.ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get session".to_string(),
        ))?;

        let session_id = session.id.clone();

        // Forward results to WebSocket
        let ws_broadcast = state.ws_broadcast.clone();
        let sid = session_id.clone();
        let executor_clone: Arc<WorkflowExecutor> = Arc::clone(&executor);

        tokio::spawn(async move {
            while let Ok(result) = result_rx.recv().await {
                let _ = ws_broadcast.send(WsEvent::ReplayStep {
                    session_id: sid.clone(),
                    result,
                });
            }

            // Send completion event
            if let Some(final_session) = executor_clone.session().await {
                let _ = ws_broadcast.send(WsEvent::ReplayComplete {
                    session_id: sid.clone(),
                    session: final_session,
                });
            }
        });

        // Store replay
        state.replays.insert(
            session_id.clone(),
            ActiveReplay::Direct {
                executor,
                session: session.clone(),
            },
        );

        tracing::info!(
            "Started direct replay session {} for workflow: {}",
            session_id,
            workflow.id
        );

        Ok(Json(StartReplayResponse {
            session_id,
            status: "running".to_string(),
        }))
    }
}

/// Stop a replay session
pub async fn stop_replay(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let (_, active) = state
        .replays
        .remove(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Replay session not found".to_string()))?;

    // Stop replay
    match active {
        ActiveReplay::Direct { executor, .. } => {
            executor.stop().await.map_err(|e| {
                tracing::error!("Failed to stop replay: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            })?;
        }
        ActiveReplay::Agent { agent, .. } => {
            agent.stop().await.map_err(|e| {
                tracing::error!("Failed to stop agent: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            })?;
        }
    }

    tracing::info!("Stopped replay session {}", session_id);

    Ok(Json(serde_json::json!({ "status": "stopped" })))
}

/// Get the status of a replay session
pub async fn get_replay_status(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionStatusResponse>, (StatusCode, String)> {
    let active = state
        .replays
        .get(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Replay session not found".to_string()))?;

    // Get latest session
    let session: Option<ReplaySession> = match &*active {
        ActiveReplay::Direct { executor, .. } => executor.session().await,
        ActiveReplay::Agent { agent, .. } => agent.session().await,
    };

    let session = session.unwrap_or_else(|| active.session().clone());

    Ok(Json(SessionStatusResponse {
        session_id: session_id.clone(),
        status: session.status,
        step_count: session.total_steps,
        current_step: session.current_step,
        error: session.error,
    }))
}
