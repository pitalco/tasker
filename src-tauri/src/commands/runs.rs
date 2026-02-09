use crate::sidecar::SidecarManager;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_instructions: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunStep {
    pub id: String,
    pub run_id: String,
    pub step_number: i64,
    pub tool_name: String,
    #[serde(default)]
    pub params: serde_json::Value,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
    pub duration_ms: i64,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunLog {
    pub id: String,
    pub run_id: String,
    pub level: String,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunListResponse {
    pub runs: Vec<Run>,
    pub total: i64,
    #[serde(default)]
    pub page: i64,
    #[serde(default)]
    pub per_page: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartRunResponse {
    pub run_id: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartRunRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headless: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_width: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_height: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<serde_json::Value>,
}

#[tauri::command]
pub async fn list_runs(
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<String>,
    workflow_id: Option<String>,
) -> Result<RunListResponse, String> {
    let client = reqwest::Client::new();
    let mut params = vec![];
    if let Some(p) = page {
        params.push(format!("page={}", p));
    }
    if let Some(pp) = per_page {
        params.push(format!("per_page={}", pp));
    }
    if let Some(s) = status {
        params.push(format!("status={}", s));
    }
    if let Some(wid) = workflow_id {
        params.push(format!("workflow_id={}", wid));
    }

    let mut url = format!("{}/runs", SidecarManager::base_url());
    if !params.is_empty() {
        url = format!("{}?{}", url, params.join("&"));
    }

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to list runs: {}", e))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Failed to list runs: {}", error_text));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

#[tauri::command]
pub async fn get_run(run_id: String) -> Result<Run, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/runs/{}", SidecarManager::base_url(), run_id);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to get run: {}", e))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Failed to get run: {}", error_text));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

#[tauri::command]
pub async fn start_run(request: StartRunRequest) -> Result<StartRunResponse, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/runs", SidecarManager::base_url());

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to start run: {}", e))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Failed to start run: {}", error_text));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

#[tauri::command]
pub async fn cancel_run(run_id: String) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/runs/{}/cancel", SidecarManager::base_url(), run_id);

    let response = client
        .post(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to cancel run: {}", e))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Failed to cancel run: {}", error_text));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

#[tauri::command]
pub async fn delete_run(run_id: String) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/runs/{}", SidecarManager::base_url(), run_id);

    let response = client
        .delete(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to delete run: {}", e))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Failed to delete run: {}", error_text));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

#[tauri::command]
pub async fn get_run_steps(run_id: String) -> Result<Vec<RunStep>, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/runs/{}/steps", SidecarManager::base_url(), run_id);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to get run steps: {}", e))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Failed to get run steps: {}", error_text));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

#[tauri::command]
pub async fn get_run_logs(run_id: String) -> Result<Vec<RunLog>, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/runs/{}/logs", SidecarManager::base_url(), run_id);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to get run logs: {}", e))?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Failed to get run logs: {}", error_text));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}
