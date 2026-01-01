use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::runs::{FileListResponse, RunFileContent};

use super::super::state::AppState;

/// Sanitize filename to prevent header injection attacks
/// Removes control characters and newlines that could be used for HTTP header injection
fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .filter(|c| !c.is_control() && *c != '\r' && *c != '\n')
        .collect()
}

/// Percent-encode filename per RFC 5987 for Content-Disposition header
fn percent_encode_filename(filename: &str) -> String {
    let mut encoded = String::new();
    for byte in filename.bytes() {
        // RFC 5987 attr-char: Allow ASCII alphanumeric and specific safe chars
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~')
        {
            encoded.push(byte as char);
        } else {
            // Percent-encode everything else
            encoded.push_str(&format!("%{:02X}", byte));
        }
    }
    encoded
}

/// Query parameters for listing files
#[derive(Debug, Deserialize)]
pub struct ListFilesQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

/// List all files with pagination
pub async fn list_files(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListFilesQuery>,
) -> Result<Json<FileListResponse>, (StatusCode, String)> {
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Repository not initialized".to_string(),
        )
    })?;

    let (files, total) = repo.list_all_files(query.limit, query.offset).map_err(|e| {
        tracing::error!("Failed to list files: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(FileListResponse { files, total }))
}

/// List files for a specific run
pub async fn list_files_for_run(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<FileListResponse>, (StatusCode, String)> {
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Repository not initialized".to_string(),
        )
    })?;

    let files = repo.list_files_for_run(&run_id).map_err(|e| {
        tracing::error!("Failed to list files for run: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    let total = files.len() as i64;
    Ok(Json(FileListResponse { files, total }))
}

/// Get file content by ID (returns base64 encoded content for API transport)
pub async fn get_file_content(
    State(state): State<Arc<AppState>>,
    Path(file_id): Path<String>,
) -> Result<Json<RunFileContent>, (StatusCode, String)> {
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Repository not initialized".to_string(),
        )
    })?;

    let file = repo
        .get_file(&file_id)
        .map_err(|e| {
            tracing::error!("Failed to get file: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "File not found".to_string()))?;

    Ok(Json(RunFileContent::from(file)))
}

/// Download file as binary (for browser download)
pub async fn download_file(
    State(state): State<Arc<AppState>>,
    Path(file_id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Repository not initialized".to_string(),
        )
    })?;

    let file = repo
        .get_file(&file_id)
        .map_err(|e| {
            tracing::error!("Failed to get file: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "File not found".to_string()))?;

    // SECURITY: Properly encode filename per RFC 5987 to prevent header injection
    // Remove any characters that could be used for header injection
    let safe_filename = sanitize_filename(&file.file_name);
    let content_disposition = format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        safe_filename.replace('"', "\\\""),
        percent_encode_filename(&file.file_name)
    );

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, &file.mime_type)
        .header(header::CONTENT_DISPOSITION, content_disposition)
        .header(header::CONTENT_LENGTH, file.file_size.to_string())
        .body(Body::from(file.content))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(response)
}

/// Delete a file
pub async fn delete_file(
    State(state): State<Arc<AppState>>,
    Path(file_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let repo = state.runs_repository.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Repository not initialized".to_string(),
        )
    })?;

    let deleted = repo.delete_file(&file_id).map_err(|e| {
        tracing::error!("Failed to delete file: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    if !deleted {
        return Err((StatusCode::NOT_FOUND, "File not found".to_string()));
    }

    tracing::info!("Deleted file {}", file_id);

    Ok(Json(serde_json::json!({
        "file_id": file_id,
        "deleted": true
    })))
}
