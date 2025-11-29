use crate::sidecar::SidecarManager;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct StartRecordingRequest {
    pub start_url: String,
    pub headless: Option<bool>,
    pub viewport_width: Option<i32>,
    pub viewport_height: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordingResponse {
    pub session_id: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordingStatusResponse {
    pub session_id: String,
    pub status: String,
    pub step_count: i32,
    pub current_step: Option<i32>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn start_sidecar() -> Result<bool, String> {
    SidecarManager::start().await?;
    Ok(true)
}

#[tauri::command]
pub async fn stop_sidecar() -> Result<bool, String> {
    SidecarManager::stop()?;
    Ok(true)
}

#[tauri::command]
pub async fn is_sidecar_running() -> Result<bool, String> {
    Ok(SidecarManager::is_running().await)
}

#[tauri::command]
pub async fn get_sidecar_urls() -> Result<(String, String), String> {
    let base_url = SidecarManager::base_url();
    let ws_url = SidecarManager::ws_url("tauri");
    Ok((base_url, ws_url))
}

#[tauri::command]
pub async fn start_recording(request: StartRecordingRequest) -> Result<RecordingResponse, String> {
    // Ensure sidecar is running
    if !SidecarManager::is_running().await {
        SidecarManager::start().await?;
    }

    let client = reqwest::Client::new();
    let url = format!("{}/recording/start", SidecarManager::base_url());

    let body = serde_json::json!({
        "start_url": request.start_url,
        "headless": request.headless.unwrap_or(false),
        "viewport_width": request.viewport_width.unwrap_or(1280),
        "viewport_height": request.viewport_height.unwrap_or(720),
    });

    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to start recording: {}", e))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Recording failed: {}", error_text));
    }

    let result: RecordingResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result)
}

#[tauri::command]
pub async fn stop_recording(session_id: String) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/recording/{}/stop",
        SidecarManager::base_url(),
        session_id
    );

    let response = client
        .post(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to stop recording: {}", e))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Stop recording failed: {}", error_text));
    }

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result)
}

#[tauri::command]
pub async fn cancel_recording(session_id: String) -> Result<bool, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/recording/{}/cancel",
        SidecarManager::base_url(),
        session_id
    );

    let response = client
        .post(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to cancel recording: {}", e))?;

    Ok(response.status().is_success())
}

#[tauri::command]
pub async fn get_recording_status(session_id: String) -> Result<RecordingStatusResponse, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/recording/{}/status",
        SidecarManager::base_url(),
        session_id
    );

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to get status: {}", e))?;

    if !response.status().is_success() {
        return Err("Session not found".to_string());
    }

    let result: RecordingStatusResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result)
}
