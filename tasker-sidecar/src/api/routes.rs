use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use super::handlers::{files, health, os, providers, recording, replay, runs, workflow};
use super::state::AppState;
use super::websocket::ws_handler;

pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health check
        .route("/health", get(health::health_check))
        // LLM Providers
        .route("/providers", get(providers::list_providers))
        // Recording endpoints
        .route("/recording/start", post(recording::start_recording))
        .route(
            "/recording/:session_id/stop",
            post(recording::stop_recording),
        )
        .route(
            "/recording/:session_id/cancel",
            post(recording::cancel_recording),
        )
        .route(
            "/recording/:session_id/status",
            get(recording::get_recording_status),
        )
        // Replay endpoints
        .route("/replay/start", post(replay::start_replay))
        .route("/replay/:session_id/stop", post(replay::stop_replay))
        .route("/replay/:session_id/status", get(replay::get_replay_status))
        // OS automation endpoints
        .route("/os/start", post(os::start_os_automation))
        .route("/os/:run_id/stop", post(os::stop_os_automation))
        // Workflow endpoints
        .route("/workflow", post(workflow::create_workflow))
        // Runs endpoints
        .route("/runs", get(runs::list_runs))
        .route("/runs", post(runs::start_run))
        .route("/runs/:run_id", get(runs::get_run))
        .route("/runs/:run_id", delete(runs::delete_run))
        .route("/runs/:run_id/cancel", post(runs::cancel_run))
        .route("/runs/:run_id/steps", get(runs::get_run_steps))
        .route("/runs/:run_id/logs", get(runs::get_run_logs))
        .route("/runs/:run_id/files", get(files::list_files_for_run))
        // Files endpoints
        .route("/files", get(files::list_files))
        .route("/files/:file_id", get(files::get_file_content))
        .route("/files/:file_id/download", get(files::download_file))
        .route("/files/:file_id", delete(files::delete_file))
        // WebSocket
        .route("/ws/:client_id", get(ws_handler))
        .layer(cors)
        .with_state(state)
}
