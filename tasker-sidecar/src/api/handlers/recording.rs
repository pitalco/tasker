use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::models::{
    RecordingSession, SessionStatusResponse, StartRecordingRequest, StartRecordingResponse,
    StopRecordingRequest, StopRecordingResponse, Viewport,
};
use crate::recording::{AIEnhancer, BrowserRecorder};

use super::super::state::{ActiveRecorder, AppState, WsEvent};

/// Start a new browser recording session
///
/// Returns immediately with status "initializing" and launches browser in background.
/// Frontend should poll `get_recording_status` until status becomes "recording" or "error".
pub async fn start_recording(
    State(state): State<Arc<AppState>>,
    Json(request): Json<StartRecordingRequest>,
) -> Result<Json<StartRecordingResponse>, (StatusCode, String)> {
    // Acquire global recording lock to prevent race conditions (double Chrome)
    let _recording_guard = state.recording_lock.lock().await;

    // Cancel any existing recording sessions first (prevents double browser)
    let existing_sessions: Vec<String> = state.recordings.iter().map(|r| r.key().clone()).collect();
    for session_id in existing_sessions {
        if let Some((_, active)) = state.recordings.remove(&session_id) {
            tracing::warn!("Cancelling existing recording session: {}", session_id);
            let _ = active.recorder.cancel().await;
        }
    }

    // Generate session ID immediately
    let session_id = Uuid::new_v4().to_string();

    // Use provided URL or blank tab
    let start_url = request.start_url.clone().unwrap_or_else(|| "about:blank".to_string());

    // Create initial session with "initializing" status
    let initial_session = RecordingSession {
        id: session_id.clone(),
        start_url: start_url.clone(),
        status: "initializing".to_string(),
        steps: vec![],
        error: None,
        started_at: Some(chrono::Utc::now()),
        completed_at: None,
    };

    // Create recorder
    let recorder = Arc::new(BrowserRecorder::new());

    // Store recorder with initializing session BEFORE launching browser
    state.recordings.insert(
        session_id.clone(),
        ActiveRecorder {
            recorder: Arc::clone(&recorder),
            session: initial_session,
            client_id: request.client_id.clone(),
        },
    );

    tracing::info!(
        "Created recording session {} (initializing) for URL: {}",
        session_id,
        start_url
    );

    // Spawn browser launch in background - this is the slow part
    let state_clone = Arc::clone(&state);
    let sid = session_id.clone();
    let start_url_for_spawn = start_url.clone();
    let headless = request.headless;
    let viewport = Some(Viewport {
        width: request.viewport_width,
        height: request.viewport_height,
    });

    tokio::spawn(async move {
        // Perform the actual browser launch
        match recorder.start(&start_url_for_spawn, headless, viewport).await {
            Ok(session) => {
                // Update session status to "recording"
                if let Some(mut active) = state_clone.recordings.get_mut(&sid) {
                    active.session = session.clone();
                    tracing::info!("Recording session {} is now active", sid);
                }

                // Subscribe to step events and forward to WebSocket
                let mut step_rx = recorder.subscribe_steps();
                let ws_broadcast = state_clone.ws_broadcast.clone();
                let sid_inner = sid.clone();

                tokio::spawn(async move {
                    while let Ok(step) = step_rx.recv().await {
                        let _ = ws_broadcast.send(WsEvent::RecordingStep {
                            session_id: sid_inner.clone(),
                            step,
                        });
                    }
                });
            }
            Err(e) => {
                // Update session status to "error"
                tracing::error!("Failed to start recording for session {}: {}", sid, e);
                if let Some(mut active) = state_clone.recordings.get_mut(&sid) {
                    active.session.status = "error".to_string();
                    active.session.error = Some(e.to_string());
                }

                // Broadcast error to WebSocket
                let _ = state_clone.ws_broadcast.send(WsEvent::Error {
                    session_id: sid.clone(),
                    error: e.to_string(),
                });
            }
        }
    });

    // Return immediately with "initializing" status
    Ok(Json(StartRecordingResponse {
        session_id,
        status: "initializing".to_string(),
    }))
}

/// Stop a recording session and generate a task description
/// AI analyzes all screenshots and actions to generate a comprehensive description
pub async fn stop_recording(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    _body: Option<Json<StopRecordingRequest>>,
) -> Result<Json<StopRecordingResponse>, (StatusCode, String)> {
    let (_, active) = state
        .recordings
        .remove(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Recording session not found".to_string()))?;

    // Get the start URL before stopping
    let start_url = active.session.start_url.clone();

    // Stop recording and get workflow
    let workflow = active.recorder.stop().await.map_err(|e| {
        tracing::error!("Failed to stop recording: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    tracing::info!(
        "Stopped recording session {}, generating task description from {} steps",
        session_id,
        workflow.steps.len()
    );

    // Generate comprehensive task description from all steps
    let (name, task_description) = if !workflow.steps.is_empty() {
        match AIEnhancer::new() {
            Some(enhancer) => {
                match enhancer.generate_task_description(&workflow.steps, &start_url).await {
                    Ok(result) => {
                        tracing::info!("Task generated for session {}: '{}'", session_id, result.name);
                        (result.name, result.description)
                    }
                    Err(e) => {
                        tracing::warn!("Task description generation failed for session {}: {}", session_id, e);
                        (
                            "Recorded Workflow".to_string(),
                            format!("Failed to generate task description: {}", e),
                        )
                    }
                }
            }
            None => {
                tracing::warn!("AI enhancer unavailable - no API key configured for default provider");
                (
                    "Recorded Workflow".to_string(),
                    "AI task description unavailable. Please configure an API key in settings.".to_string(),
                )
            }
        }
    } else {
        ("Empty Recording".to_string(), "No actions were recorded.".to_string())
    };

    Ok(Json(StopRecordingResponse { name, task_description }))
}

/// Cancel a recording session without saving
pub async fn cancel_recording(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let (_, active) = state
        .recordings
        .remove(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Recording session not found".to_string()))?;

    // Cancel recording
    active.recorder.cancel().await.map_err(|e| {
        tracing::error!("Failed to cancel recording: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    tracing::info!("Cancelled recording session {}", session_id);

    Ok(Json(serde_json::json!({ "status": "cancelled" })))
}

/// Get the status of a recording session
///
/// Returns the current status of the recording session:
/// - "initializing": Browser is still launching
/// - "recording": Browser is ready and recording
/// - "paused": Recording is paused
/// - "error": An error occurred during initialization or recording
pub async fn get_recording_status(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionStatusResponse>, (StatusCode, String)> {
    let active = state
        .recordings
        .get(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Recording session not found".to_string()))?;

    // Get status from stored session (updated by background task)
    // This shows "initializing" while browser is launching, "recording" when ready
    let stored_status = active.session.status.clone();
    let stored_error = active.session.error.clone();

    // Get step count from recorder if available
    let step_count = active.recorder.step_count().await;

    Ok(Json(SessionStatusResponse {
        session_id: session_id.clone(),
        status: stored_status,
        step_count: step_count as i32,
        current_step: step_count as i32,
        error: stored_error,
    }))
}
