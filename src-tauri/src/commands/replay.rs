use crate::sidecar::SidecarManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request to start a replay - AI agent is ALWAYS used
#[derive(Debug, Serialize, Deserialize)]
pub struct StartReplayRequest {
    pub workflow: serde_json::Value,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub task_description: Option<String>,
    pub variables: Option<HashMap<String, serde_json::Value>>,
    pub iterations: Option<i32>,
    pub headless: Option<bool>,
    /// Optional condition - agent will NOT stop until this is met
    pub stop_when: Option<String>,
    /// Max steps override (None = use global default)
    pub max_steps: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplayResponse {
    pub session_id: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplayStatusResponse {
    pub session_id: String,
    pub status: String,
    pub step_count: i32,
    pub current_step: i32,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProvidersResponse {
    pub providers: HashMap<String, Vec<String>>,
}

#[tauri::command]
pub async fn get_llm_providers() -> Result<ProvidersResponse, String> {
    // Ensure sidecar is running
    if !SidecarManager::is_running().await {
        SidecarManager::start().await?;
    }

    let client = reqwest::Client::new();
    let url = format!("{}/providers", SidecarManager::base_url());

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to get providers: {}", e))?;

    if !response.status().is_success() {
        return Err("Failed to get providers".to_string());
    }

    let result: ProvidersResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result)
}

#[tauri::command]
pub async fn start_replay(request: StartReplayRequest) -> Result<ReplayResponse, String> {
    // Ensure sidecar is running
    if !SidecarManager::is_running().await {
        SidecarManager::start().await?;
    }

    let client = reqwest::Client::new();
    let url = format!("{}/replay/start", SidecarManager::base_url());

    // Transform workflow to match sidecar's expected format
    let mut workflow = request.workflow.clone();
    if let Some(obj) = workflow.as_object_mut() {
        // Convert variables from array to map format
        if let Some(vars_array) = obj.get("variables").and_then(|v| v.as_array()) {
            let vars_map: HashMap<String, serde_json::Value> = vars_array
                .iter()
                .filter_map(|v| {
                    let name = v.get("name")?.as_str()?;
                    let default = v.get("default_value").cloned().unwrap_or(serde_json::Value::Null);
                    Some((name.to_string(), default))
                })
                .collect();
            obj.insert("variables".to_string(), serde_json::to_value(vars_map).unwrap());
        }
    }

    let body = serde_json::json!({
        "workflow": workflow,
        "llm_provider": request.llm_provider.unwrap_or_else(|| "google".to_string()),
        "llm_model": request.llm_model.unwrap_or_else(|| "gemini-3-pro-preview".to_string()),
        "task_description": request.task_description,
        "variables": request.variables.unwrap_or_default(),
        "iterations": request.iterations.unwrap_or(1),
        "headless": request.headless.unwrap_or(false),
        "stop_when": request.stop_when,
        "max_steps": request.max_steps,
    });

    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to start replay: {}", e))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Replay failed: {}", error_text));
    }

    let result: ReplayResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result)
}

#[tauri::command]
pub async fn stop_replay(session_id: String) -> Result<bool, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/replay/{}/stop", SidecarManager::base_url(), session_id);

    let response = client
        .post(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to stop replay: {}", e))?;

    Ok(response.status().is_success())
}

#[tauri::command]
pub async fn get_replay_status(session_id: String) -> Result<ReplayStatusResponse, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/replay/{}/status",
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

    let result: ReplayStatusResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result)
}

// ============================================================================
// OS Automation Commands
// ============================================================================

/// Request to start OS automation
#[derive(Debug, Serialize, Deserialize)]
pub struct StartOsRequest {
    pub task_description: String,
    pub custom_instructions: Option<String>,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub max_steps: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OsResponse {
    pub run_id: String,
    pub status: String,
}

#[tauri::command]
pub async fn start_os_automation(request: StartOsRequest) -> Result<OsResponse, String> {
    // Ensure sidecar is running
    if !SidecarManager::is_running().await {
        SidecarManager::start().await?;
    }

    let client = reqwest::Client::new();
    let url = format!("{}/os/start", SidecarManager::base_url());

    let body = serde_json::json!({
        "task_description": request.task_description,
        "custom_instructions": request.custom_instructions,
        "llm_provider": request.llm_provider.unwrap_or_else(|| "anthropic".to_string()),
        "llm_model": request.llm_model.unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
        "max_steps": request.max_steps,
    });

    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to start OS automation: {}", e))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("OS automation failed: {}", error_text));
    }

    let result: OsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result)
}

#[tauri::command]
pub async fn stop_os_automation(run_id: String) -> Result<bool, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/os/{}/stop", SidecarManager::base_url(), run_id);

    let response = client
        .post(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to stop OS automation: {}", e))?;

    Ok(response.status().is_success())
}
