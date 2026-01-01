use serde::Deserialize;
use std::collections::HashMap;

use super::workflow::Workflow;

#[derive(Debug, Deserialize)]
pub struct StartRecordingRequest {
    /// Optional start URL - if not provided, opens a blank tab
    pub start_url: Option<String>,
    #[serde(default)]
    pub headless: bool,
    #[serde(default = "default_viewport_width")]
    pub viewport_width: i32,
    #[serde(default = "default_viewport_height")]
    pub viewport_height: i32,
    /// Optional client ID for tracking which client started the recording
    /// Used for cleanup when client disconnects
    pub client_id: Option<String>,
}

fn default_viewport_width() -> i32 {
    1280
}
fn default_viewport_height() -> i32 {
    720
}

/// Request to start a replay - AI agent is ALWAYS used
/// Recorded workflow serves as hints/context for the AI, not strict instructions
#[derive(Debug, Deserialize)]
pub struct StartReplayRequest {
    pub workflow: Workflow,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub task_description: Option<String>,
    #[serde(default)]
    pub variables: HashMap<String, serde_json::Value>,
    #[serde(default = "default_iterations")]
    pub iterations: i32,
    #[serde(default)]
    pub headless: bool,
    /// Optional condition - agent will NOT stop until this is met
    pub stop_when: Option<String>,
    /// Max steps override (None = use global default)
    pub max_steps: Option<i32>,
}

fn default_iterations() -> i32 {
    1
}

/// Request to stop a recording session and generate task description
#[derive(Debug, Deserialize, Default)]
pub struct StopRecordingRequest {}
