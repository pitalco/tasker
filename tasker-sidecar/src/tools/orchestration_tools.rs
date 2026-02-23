use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;

use super::registry::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Maximum agent nesting depth to prevent infinite recursion
const MAX_AGENT_DEPTH: u32 = 5;

/// Spawn a child agent to handle a subtask
pub struct SpawnAgentTool;

#[async_trait]
impl Tool for SpawnAgentTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "spawn_agent".to_string(),
            description: "Start a new child agent to handle a subtask. The child gets its own browser and runs independently. Returns a run_id to track it.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "task_description": {
                        "type": "string",
                        "description": "Detailed description of the task for the child agent to complete"
                    },
                    "workflow_id": {
                        "type": "string",
                        "description": "Optional workflow ID to use as a template for the child agent"
                    },
                    "variables": {
                        "type": "object",
                        "description": "Optional variables to pass to the child agent"
                    }
                },
                "required": ["task_description"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let task_description = match params["task_description"].as_str() {
            Some(t) => t.to_string(),
            None => return Ok(ToolResult::error("Missing 'task_description' parameter")),
        };

        let spawner = match &ctx.run_spawner {
            Some(s) => s.clone(),
            None => {
                return Ok(ToolResult::error(
                    "Agent orchestration is not available in this context",
                ))
            }
        };

        // Check depth limit
        if ctx.agent_depth >= MAX_AGENT_DEPTH {
            return Ok(ToolResult::error(format!(
                "Maximum agent nesting depth ({}) reached. Cannot spawn more child agents.",
                MAX_AGENT_DEPTH
            )));
        }

        let workflow_id = params["workflow_id"].as_str().map(|s| s.to_string());

        let variables: Option<HashMap<String, String>> = params["variables"]
            .as_object()
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            });

        match spawner
            .spawn_run(task_description.clone(), workflow_id, variables, ctx.agent_depth)
            .await
        {
            Ok(run_id) => Ok(ToolResult::success_with_data(
                format!(
                    "Child agent spawned successfully. Run ID: {}. Use await_agent or get_agent_status to check progress.",
                    run_id
                ),
                json!({ "run_id": run_id }),
            )),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to spawn child agent: {}",
                e
            ))),
        }
    }
}

/// Block until a child agent run completes
pub struct AwaitAgentTool;

#[async_trait]
impl Tool for AwaitAgentTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "await_agent".to_string(),
            description: "Wait for a child agent run to complete. Polls every 2 seconds. Returns the result when done.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "run_id": {
                        "type": "string",
                        "description": "The run ID of the child agent to wait for"
                    },
                    "timeout_seconds": {
                        "type": "integer",
                        "description": "Maximum time to wait in seconds (default: 300)",
                        "default": 300
                    }
                },
                "required": ["run_id"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let run_id = match params["run_id"].as_str() {
            Some(r) => r.to_string(),
            None => return Ok(ToolResult::error("Missing 'run_id' parameter")),
        };

        let timeout_secs = params["timeout_seconds"]
            .as_i64()
            .unwrap_or(300)
            .max(1)
            .min(600) as u64;

        let spawner = match &ctx.run_spawner {
            Some(s) => s.clone(),
            None => {
                return Ok(ToolResult::error(
                    "Agent orchestration is not available in this context",
                ))
            }
        };

        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);

        loop {
            match spawner.get_run_status(&run_id).await {
                Ok(status) => {
                    let is_done = matches!(
                        status.status.as_str(),
                        "completed" | "failed" | "cancelled"
                    );

                    if is_done {
                        return Ok(ToolResult::success_with_data(
                            format!(
                                "Child agent {} finished with status: {}",
                                run_id, status.status
                            ),
                            json!({
                                "run_id": status.run_id,
                                "status": status.status,
                                "result": status.result,
                                "error": status.error,
                            }),
                        ));
                    }

                    if tokio::time::Instant::now() >= deadline {
                        return Ok(ToolResult::error(format!(
                            "Timed out waiting for child agent {} after {} seconds. Current status: {}",
                            run_id, timeout_secs, status.status
                        )));
                    }

                    // Poll every 2 seconds
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
                Err(e) => {
                    return Ok(ToolResult::error(format!(
                        "Failed to check status of child agent {}: {}",
                        run_id, e
                    )));
                }
            }
        }
    }
}

/// Non-blocking check of a child agent's status
pub struct GetAgentStatusTool;

#[async_trait]
impl Tool for GetAgentStatusTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_agent_status".to_string(),
            description: "Check the status of a child agent run without blocking. Returns status and result if done.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "run_id": {
                        "type": "string",
                        "description": "The run ID of the child agent to check"
                    }
                },
                "required": ["run_id"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let run_id = match params["run_id"].as_str() {
            Some(r) => r.to_string(),
            None => return Ok(ToolResult::error("Missing 'run_id' parameter")),
        };

        let spawner = match &ctx.run_spawner {
            Some(s) => s.clone(),
            None => {
                return Ok(ToolResult::error(
                    "Agent orchestration is not available in this context",
                ))
            }
        };

        match spawner.get_run_status(&run_id).await {
            Ok(status) => Ok(ToolResult::success_with_data(
                format!("Agent {} status: {}", run_id, status.status),
                json!({
                    "run_id": status.run_id,
                    "status": status.status,
                    "result": status.result,
                    "error": status.error,
                }),
            )),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to get status for agent {}: {}",
                run_id, e
            ))),
        }
    }
}

/// List all active agent runs
pub struct ListAgentsTool;

#[async_trait]
impl Tool for ListAgentsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_agents".to_string(),
            description: "List all active agent runs with their IDs, tasks, and statuses.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(&self, _params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let spawner = match &ctx.run_spawner {
            Some(s) => s.clone(),
            None => {
                return Ok(ToolResult::error(
                    "Agent orchestration is not available in this context",
                ))
            }
        };

        match spawner.list_active_runs().await {
            Ok(runs) => {
                if runs.is_empty() {
                    return Ok(ToolResult::success("No active agent runs"));
                }

                let runs_json: Vec<Value> = runs
                    .iter()
                    .map(|r| {
                        json!({
                            "run_id": r.run_id,
                            "task": r.task_description,
                            "status": r.status,
                        })
                    })
                    .collect();

                Ok(ToolResult::success_with_data(
                    format!("{} active agent run(s)", runs.len()),
                    json!({ "agents": runs_json }),
                ))
            }
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to list agents: {}",
                e
            ))),
        }
    }
}
