use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;

/// Run status enum
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    #[default]
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Pending => "pending",
            RunStatus::Running => "running",
            RunStatus::Completed => "completed",
            RunStatus::Failed => "failed",
            RunStatus::Cancelled => "cancelled",
        }
    }
}

impl FromStr for RunStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(RunStatus::Pending),
            "running" => Ok(RunStatus::Running),
            "completed" => Ok(RunStatus::Completed),
            "failed" => Ok(RunStatus::Failed),
            "cancelled" => Ok(RunStatus::Cancelled),
            _ => Err(()),
        }
    }
}

/// Log level enum
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

impl FromStr for LogLevel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(()),
        }
    }
}

/// A workflow run - represents a single execution of a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_name: Option<String>,
    pub status: RunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_instructions: Option<String>,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The final result/response from the agent (markdown formatted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(default)]
    pub metadata: Value,
    /// Steps executed in this run (populated when fetching run details)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<RunStep>,
    /// Logs for this run (populated when fetching run details)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<RunLog>,
}

impl Run {
    /// Create a new run
    pub fn new(
        workflow_id: Option<String>,
        workflow_name: Option<String>,
        task_description: Option<String>,
        custom_instructions: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            workflow_id,
            workflow_name,
            status: RunStatus::Pending,
            task_description,
            custom_instructions,
            started_at: Utc::now(),
            completed_at: None,
            error: None,
            result: None,
            metadata: Value::Object(serde_json::Map::new()),
            steps: Vec::new(),
            logs: Vec::new(),
        }
    }

    /// Start the run
    pub fn start(&mut self) {
        self.status = RunStatus::Running;
        self.started_at = Utc::now();
    }

    /// Complete the run successfully
    pub fn complete(&mut self) {
        self.status = RunStatus::Completed;
        self.completed_at = Some(Utc::now());
    }

    /// Fail the run with an error
    pub fn fail(&mut self, error: String) {
        self.status = RunStatus::Failed;
        self.error = Some(error);
        self.completed_at = Some(Utc::now());
    }

    /// Cancel the run
    pub fn cancel(&mut self) {
        self.status = RunStatus::Cancelled;
        self.completed_at = Some(Utc::now());
    }

    /// Get the duration in milliseconds
    pub fn duration_ms(&self) -> Option<i64> {
        self.completed_at.map(|completed| {
            (completed - self.started_at).num_milliseconds()
        })
    }

    /// Check if the run is finished (completed, failed, or cancelled)
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled
        )
    }
}

/// A single step executed within a run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStep {
    pub id: String,
    pub run_id: String,
    pub step_number: i32,
    pub tool_name: String,
    pub params: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration_ms: i64,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
}

impl RunStep {
    /// Create a new run step
    pub fn new(run_id: String, step_number: i32, tool_name: String, params: Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            run_id,
            step_number,
            tool_name,
            params,
            result: None,
            success: false,
            error: None,
            duration_ms: 0,
            timestamp: Utc::now(),
            screenshot: None,
        }
    }

    /// Mark step as successful with result
    pub fn succeed(&mut self, result: Option<Value>, duration_ms: i64) {
        self.success = true;
        self.result = result;
        self.duration_ms = duration_ms;
    }

    /// Mark step as failed with error
    pub fn fail(&mut self, error: String, duration_ms: i64) {
        self.success = false;
        self.error = Some(error);
        self.duration_ms = duration_ms;
    }

    /// Complete the step with result
    pub fn complete(
        &mut self,
        success: bool,
        result: Option<String>,
        error: Option<String>,
        duration_ms: i64,
    ) {
        self.success = success;
        self.result = result.map(serde_json::Value::String);
        self.error = error;
        self.duration_ms = duration_ms;
    }
}

/// A log entry for a run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunLog {
    pub id: String,
    pub run_id: String,
    pub level: LogLevel,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    pub timestamp: DateTime<Utc>,
}

impl RunLog {
    /// Create a new log entry
    pub fn new(run_id: String, level: LogLevel, message: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            run_id,
            level,
            message,
            metadata: None,
            timestamp: Utc::now(),
        }
    }

    /// Create a debug log
    pub fn debug(run_id: String, message: String) -> Self {
        Self::new(run_id, LogLevel::Debug, message)
    }

    /// Create an info log
    pub fn info(run_id: String, message: String) -> Self {
        Self::new(run_id, LogLevel::Info, message)
    }

    /// Create a warning log
    pub fn warn(run_id: String, message: String) -> Self {
        Self::new(run_id, LogLevel::Warn, message)
    }

    /// Create an error log
    pub fn error(run_id: String, message: String) -> Self {
        Self::new(run_id, LogLevel::Error, message)
    }

    /// Add metadata to the log
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Request to create a new run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRunRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_instructions: Option<String>,
    #[serde(default)]
    pub variables: serde_json::Map<String, Value>,
    #[serde(default)]
    pub headless: bool,
}

/// Response for run list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunListResponse {
    pub runs: Vec<Run>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}

/// Query parameters for listing runs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<RunStatus>,
    #[serde(default = "default_page")]
    pub page: i32,
    #[serde(default = "default_per_page")]
    pub per_page: i32,
    #[serde(default)]
    pub sort_desc: bool,
}

fn default_page() -> i32 {
    1
}

fn default_per_page() -> i32 {
    20
}
