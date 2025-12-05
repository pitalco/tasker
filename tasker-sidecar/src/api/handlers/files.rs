use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::runs::{FileListResponse, RunFileContent, RunFileMetadata};

use super::super::state::AppState;

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
) -> Result<Json<Vec<RunFileMetadata>>, (StatusCode, String)> {
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

    Ok(Json(files))
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

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, &file.mime_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", file.file_name),
        )
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
