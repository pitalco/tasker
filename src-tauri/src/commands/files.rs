use crate::sidecar::SidecarManager;
use serde::{Deserialize, Serialize};
use std::fs;
use tauri_plugin_dialog::DialogExt;

/// File metadata returned from API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub id: String,
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    pub file_name: String,
    pub file_path: String,
    pub mime_type: String,
    pub file_size: i64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_name: Option<String>,
}

/// Response for listing files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListResponse {
    pub files: Vec<FileMetadata>,
    pub total: i64,
}

/// File content response (with base64 encoded content)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContentResponse {
    pub id: String,
    pub file_name: String,
    pub file_path: String,
    pub mime_type: String,
    pub file_size: i64,
    pub content_base64: String,
}

/// List all files with pagination
#[tauri::command]
pub async fn get_all_files(limit: Option<i64>, offset: Option<i64>) -> Result<FileListResponse, String> {
    let client = reqwest::Client::new();
    let mut url = format!("{}/files", SidecarManager::base_url());

    // Add query parameters
    let mut params = vec![];
    if let Some(l) = limit {
        params.push(format!("limit={}", l));
    }
    if let Some(o) = offset {
        params.push(format!("offset={}", o));
    }
    if !params.is_empty() {
        url = format!("{}?{}", url, params.join("&"));
    }

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch files: {}", e))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Failed to fetch files: {}", error_text));
    }

    let result: FileListResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result)
}

/// List files for a specific run
#[tauri::command]
pub async fn get_files_for_run(run_id: String) -> Result<Vec<FileMetadata>, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/runs/{}/files", SidecarManager::base_url(), run_id);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch files: {}", e))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Failed to fetch files: {}", error_text));
    }

    let result: Vec<FileMetadata> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result)
}

/// Get file content by ID (returns base64 encoded content)
#[tauri::command]
pub async fn get_file_content(file_id: String) -> Result<FileContentResponse, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/files/{}", SidecarManager::base_url(), file_id);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch file: {}", e))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Failed to fetch file: {}", error_text));
    }

    let result: FileContentResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result)
}

/// Delete a file
#[tauri::command]
pub async fn delete_file(file_id: String) -> Result<bool, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/files/{}", SidecarManager::base_url(), file_id);

    let response = client
        .delete(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to delete file: {}", e))?;

    Ok(response.status().is_success())
}

/// Download a file using native save dialog
#[tauri::command]
pub async fn download_file(
    app: tauri::AppHandle,
    file_id: String,
    suggested_name: String,
) -> Result<bool, String> {
    // Fetch file content from sidecar
    let client = reqwest::Client::new();
    let url = format!("{}/files/{}/download", SidecarManager::base_url(), file_id);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch file: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("File not found: {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read file content: {}", e))?;

    // Show save dialog
    let file_path = app.dialog()
        .file()
        .set_file_name(&suggested_name)
        .blocking_save_file();

    match file_path {
        Some(path) => {
            fs::write(path.as_path().unwrap(), bytes)
                .map_err(|e| format!("Failed to write file: {}", e))?;
            Ok(true)
        }
        None => Ok(false), // User cancelled
    }
}
