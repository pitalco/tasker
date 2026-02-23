use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::browser::{BrowserManager, SelectorMap};
use crate::runs::RunRepository;

/// A memory/note stored during a run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Optional key for easy reference (e.g., "user_email", "search_count")
    pub key: Option<String>,
    /// The memory content
    pub content: String,
    /// Optional category (e.g., "observation", "extracted_data")
    pub category: Option<String>,
}

impl Memory {
    pub fn new(content: impl Into<String>, key: Option<String>, category: Option<String>) -> Self {
        Self {
            key,
            content: content.into(),
            category,
        }
    }
}

/// Tool definition for LLM function calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value, // JSON Schema
}

/// Result from tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    /// If true, the task is complete
    #[serde(default)]
    pub is_done: bool,
}

impl ToolResult {
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: Some(content.into()),
            error: None,
            data: None,
            is_done: false,
        }
    }

    pub fn success_with_data(content: impl Into<String>, data: Value) -> Self {
        Self {
            success: true,
            content: Some(content.into()),
            error: None,
            data: Some(data),
            is_done: false,
        }
    }

    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            content: None,
            error: Some(error.into()),
            data: None,
            is_done: false,
        }
    }

    pub fn done(content: impl Into<String>, success: bool) -> Self {
        Self {
            success,
            content: Some(content.into()),
            error: None,
            data: None,
            is_done: true,
        }
    }
}

/// Terminal session state shared across commands within a run.
/// Uses per-command execution with shared working directory and environment variables.
pub struct TerminalSession {
    /// Current working directory (persists across commands within a run)
    pub working_directory: PathBuf,
    /// Environment variables for commands
    pub env_vars: HashMap<String, String>,
    /// Background command handle (if any)
    pub background_child: Option<tokio::process::Child>,
    /// Accumulated output from background command
    pub background_output: String,
    /// Whether a background command is still running
    pub background_running: bool,
}

impl TerminalSession {
    pub fn new() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            env_vars: HashMap::new(),
            background_child: None,
            background_output: String::new(),
            background_running: false,
        }
    }
}

impl Default for TerminalSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for spawning and managing child agent runs (orchestration).
#[async_trait]
pub trait RunSpawner: Send + Sync {
    /// Spawn a new child agent run. Returns the run_id.
    async fn spawn_run(
        &self,
        task_description: String,
        workflow_id: Option<String>,
        variables: Option<HashMap<String, String>>,
        parent_depth: u32,
    ) -> Result<String>;

    /// Get the status of a run (non-blocking).
    async fn get_run_status(&self, run_id: &str) -> Result<RunSpawnerStatus>;

    /// List all active runs.
    async fn list_active_runs(&self) -> Result<Vec<RunSpawnerInfo>>;
}

/// Status info returned by RunSpawner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSpawnerStatus {
    pub run_id: String,
    pub status: String,
    pub result: Option<String>,
    pub error: Option<String>,
}

/// Info about an active run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSpawnerInfo {
    pub run_id: String,
    pub task_description: Option<String>,
    pub status: String,
}

/// Context passed to tools during execution
pub struct ToolContext {
    pub run_id: String,
    /// Workflow ID (if running a workflow)
    pub workflow_id: Option<String>,
    pub browser: Arc<BrowserManager>,
    /// Current selector map from the page (updated before each LLM turn)
    pub selector_map: Arc<RwLock<SelectorMap>>,
    /// Repository for file storage operations
    pub file_repository: Option<Arc<RunRepository>>,
    /// In-memory storage for notes/memories during this run
    pub memories: Arc<RwLock<Vec<Memory>>>,
    /// Terminal session for command execution (lazy-init per run)
    pub terminal_session: Arc<tokio::sync::Mutex<TerminalSession>>,
    /// Allowed directories for real filesystem access (empty = no access)
    pub allowed_directories: Vec<PathBuf>,
    /// Spawner for creating child agent runs (orchestration)
    pub run_spawner: Option<Arc<dyn RunSpawner>>,
    /// Current agent nesting depth (0 = top-level)
    pub agent_depth: u32,
}

/// Trait for implementing tools
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool definition for LLM function calling
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with given parameters
    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult>;
}

/// Registry of all available tools
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let def = tool.definition();
        self.tools.insert(def.name.clone(), tool);
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// Get all tool definitions for LLM function calling
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Execute a tool by name
    pub async fn execute(&self, name: &str, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        match self.tools.get(name) {
            Some(tool) => tool.execute(params, ctx).await,
            None => Ok(ToolResult::error(format!("Unknown tool: {}", name))),
        }
    }

    /// Convert tool definitions to Claude tool_use format
    pub fn to_claude_tools(&self) -> Vec<Value> {
        self.definitions()
            .into_iter()
            .map(|def| {
                serde_json::json!({
                    "name": def.name,
                    "description": def.description,
                    "input_schema": def.parameters
                })
            })
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
