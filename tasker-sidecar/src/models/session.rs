use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::workflow::WorkflowStep;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSession {
    pub id: String,
    pub start_url: String,
    #[serde(default = "default_status")]
    pub status: String, // "pending", "recording", "paused", "completed", "error"
    #[serde(default)]
    pub steps: Vec<WorkflowStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
}

fn default_status() -> String {
    "pending".to_string()
}

impl RecordingSession {
    pub fn new(start_url: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            start_url,
            status: "pending".to_string(),
            steps: Vec::new(),
            error: None,
            started_at: None,
            completed_at: None,
        }
    }

    pub fn start(&mut self) {
        self.status = "recording".to_string();
        self.started_at = Some(Utc::now());
    }

    pub fn pause(&mut self) {
        self.status = "paused".to_string();
    }

    pub fn resume(&mut self) {
        self.status = "recording".to_string();
    }

    pub fn complete(&mut self) {
        self.status = "completed".to_string();
        self.completed_at = Some(Utc::now());
    }

    pub fn fail(&mut self, error: String) {
        self.status = "error".to_string();
        self.error = Some(error);
        self.completed_at = Some(Utc::now());
    }

    pub fn add_step(&mut self, step: WorkflowStep) {
        self.steps.push(step);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_params: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub duration_ms: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extracted_data: Option<HashMap<String, serde_json::Value>>,
}

impl StepResult {
    pub fn success(step_id: String, duration_ms: i32) -> Self {
        Self {
            step_id,
            success: true,
            tool_name: None,
            tool_params: None,
            screenshot: None,
            error: None,
            duration_ms,
            extracted_data: None,
        }
    }

    pub fn success_with_tool(step_id: String, duration_ms: i32, tool_name: String, tool_params: String) -> Self {
        Self {
            step_id,
            success: true,
            tool_name: Some(tool_name),
            tool_params: Some(tool_params),
            screenshot: None,
            error: None,
            duration_ms,
            extracted_data: None,
        }
    }

    pub fn failure(step_id: String, error: String) -> Self {
        Self {
            step_id,
            success: false,
            tool_name: None,
            tool_params: None,
            screenshot: None,
            error: Some(error),
            duration_ms: 0,
            extracted_data: None,
        }
    }

    pub fn failure_with_tool(step_id: String, error: String, tool_name: String, tool_params: String) -> Self {
        Self {
            step_id,
            success: false,
            tool_name: Some(tool_name),
            tool_params: Some(tool_params),
            screenshot: None,
            error: Some(error),
            duration_ms: 0,
            extracted_data: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaySession {
    pub id: String,
    pub workflow_id: String,
    #[serde(default = "default_status")]
    pub status: String, // "pending", "running", "paused", "completed", "error"
    #[serde(default)]
    pub current_step: i32,
    #[serde(default)]
    pub total_steps: i32,
    #[serde(default)]
    pub results: Vec<StepResult>,
    #[serde(default)]
    pub variables: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
}

impl ReplaySession {
    pub fn new(workflow_id: String, total_steps: i32, variables: HashMap<String, serde_json::Value>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            workflow_id,
            status: "pending".to_string(),
            current_step: 0,
            total_steps,
            results: Vec::new(),
            variables,
            error: None,
            started_at: None,
            completed_at: None,
        }
    }

    pub fn start(&mut self) {
        self.status = "running".to_string();
        self.started_at = Some(Utc::now());
    }

    pub fn complete(&mut self) {
        self.status = "completed".to_string();
        self.completed_at = Some(Utc::now());
    }

    pub fn fail(&mut self, error: String) {
        self.status = "error".to_string();
        self.error = Some(error);
        self.completed_at = Some(Utc::now());
    }

    pub fn add_result(&mut self, result: StepResult) {
        self.results.push(result);
        self.current_step += 1;
    }
}
