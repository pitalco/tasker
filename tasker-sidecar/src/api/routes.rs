use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use super::handlers::{health, providers, recording, replay, runs};
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
        // Runs endpoints
        .route("/runs", get(runs::list_runs))
        .route("/runs", post(runs::start_run))
        .route("/runs/:run_id", get(runs::get_run))
        .route("/runs/:run_id", delete(runs::delete_run))
        .route("/runs/:run_id/cancel", post(runs::cancel_run))
        .route("/runs/:run_id/steps", get(runs::get_run_steps))
        .route("/runs/:run_id/logs", get(runs::get_run_logs))
        // WebSocket
        .route("/ws/:client_id", get(ws_handler))
        .layer(cors)
        .with_state(state)
}
