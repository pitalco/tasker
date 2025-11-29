use serde::Serialize;
use std::collections::HashMap;

use super::workflow::Workflow;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ProvidersResponse {
    pub providers: HashMap<String, Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct StartRecordingResponse {
    pub session_id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct StopRecordingResponse {
    pub workflow: Workflow,
}

#[derive(Debug, Serialize)]
pub struct StartReplayResponse {
    pub session_id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct SessionStatusResponse {
    pub session_id: String,
    pub status: String,
    #[serde(default)]
    pub step_count: i32,
    #[serde(default)]
    pub current_step: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GenericResponse {
    pub status: String,
}
