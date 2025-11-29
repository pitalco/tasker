use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Browser automation error: {0}")]
    BrowserError(String),

    #[error("LLM provider error: {0}")]
    LLMError(String),

    #[error("Invalid request: {0}")]
    ValidationError(String),

    #[error("Recording error: {0}")]
    RecordingError(String),

    #[error("Replay error: {0}")]
    ReplayError(String),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    detail: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AppError::SessionNotFound(_) => (StatusCode::NOT_FOUND, "Not Found"),
            AppError::ValidationError(_) => (StatusCode::BAD_REQUEST, "Bad Request"),
            AppError::BrowserError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Browser Error"),
            AppError::LLMError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "LLM Error"),
            AppError::RecordingError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Recording Error"),
            AppError::ReplayError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Replay Error"),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Error"),
        };

        let body = Json(ErrorResponse {
            error: error_message.to_string(),
            detail: self.to_string(),
        });

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
