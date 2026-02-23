use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use super::registry::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Execute a command and return stdout/stderr/exit_code.
/// Uses per-command execution with shared working directory from TerminalSession.
pub struct ExecuteCommandTool;

#[async_trait]
impl Tool for ExecuteCommandTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "execute_command".to_string(),
            description: "Execute a shell command and return stdout, stderr, and exit code. The working directory persists across commands within a run.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    },
                    "timeout_seconds": {
                        "type": "integer",
                        "description": "Timeout in seconds (default: 30, max: 300)",
                        "default": 30
                    },
                    "working_directory": {
                        "type": "string",
                        "description": "Optional override for the working directory. If provided, also updates the session's working directory."
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let command = match params["command"].as_str() {
            Some(c) => c.to_string(),
            None => return Ok(ToolResult::error("Missing 'command' parameter")),
        };

        let timeout_secs = params["timeout_seconds"]
            .as_i64()
            .unwrap_or(30)
            .max(1)
            .min(300) as u64;

        // Get/update working directory from session
        let working_dir = {
            let mut session = ctx.terminal_session.lock().await;

            if let Some(wd) = params["working_directory"].as_str() {
                let path = PathBuf::from(wd);
                if path.is_dir() {
                    session.working_directory = path;
                } else {
                    return Ok(ToolResult::error(format!(
                        "Working directory does not exist: {}",
                        wd
                    )));
                }
            }

            session.working_directory.clone()
        };

        // Build the command based on platform
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd.exe");
            c.args(["/C", &command]);
            c
        } else {
            let mut c = Command::new("/bin/bash");
            c.args(["-c", &command]);
            c
        };

        cmd.current_dir(&working_dir);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // Inject session env vars
        {
            let session = ctx.terminal_session.lock().await;
            for (k, v) in &session.env_vars {
                cmd.env(k, v);
            }
        }

        // Execute with timeout
        let result = tokio::time::timeout(
            tokio::time::Duration::from_secs(timeout_secs),
            cmd.output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let exit_code = output.status.code().unwrap_or(-1);

                // Check if the command was a cd command and update working directory
                let trimmed = command.trim();
                if trimmed.starts_with("cd ") || trimmed.starts_with("cd\t") {
                    let target = trimmed[3..].trim().trim_matches('"').trim_matches('\'');
                    let new_dir = if PathBuf::from(target).is_absolute() {
                        PathBuf::from(target)
                    } else {
                        working_dir.join(target)
                    };
                    if let Ok(canonical) = std::fs::canonicalize(&new_dir) {
                        let mut session = ctx.terminal_session.lock().await;
                        session.working_directory = canonical;
                    }
                }

                // Truncate very long output
                let max_len = 50_000;
                let stdout_display = if stdout.len() > max_len {
                    format!("{}...\n[Output truncated, {} total chars]", &stdout[..max_len], stdout.len())
                } else {
                    stdout.to_string()
                };
                let stderr_display = if stderr.len() > max_len {
                    format!("{}...\n[Output truncated, {} total chars]", &stderr[..max_len], stderr.len())
                } else {
                    stderr.to_string()
                };

                let content = format!(
                    "Exit code: {}\n\n--- stdout ---\n{}\n--- stderr ---\n{}",
                    exit_code,
                    stdout_display.trim(),
                    stderr_display.trim()
                );

                if exit_code == 0 {
                    Ok(ToolResult::success(content))
                } else {
                    Ok(ToolResult::success_with_data(
                        content,
                        json!({ "exit_code": exit_code }),
                    ))
                }
            }
            Ok(Err(e)) => Ok(ToolResult::error(format!("Failed to execute command: {}", e))),
            Err(_) => Ok(ToolResult::error(format!(
                "Command timed out after {} seconds",
                timeout_secs
            ))),
        }
    }
}

/// Start a long-running command in the background.
pub struct ExecuteCommandBackgroundTool;

#[async_trait]
impl Tool for ExecuteCommandBackgroundTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "execute_command_background".to_string(),
            description: "Start a long-running shell command in the background. Returns immediately. Use read_terminal_output to check progress.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to run in the background"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let command = match params["command"].as_str() {
            Some(c) => c.to_string(),
            None => return Ok(ToolResult::error("Missing 'command' parameter")),
        };

        let mut session = ctx.terminal_session.lock().await;

        // Kill any existing background process
        if let Some(ref mut child) = session.background_child {
            let _ = child.kill().await;
        }
        session.background_output.clear();
        session.background_running = false;

        let working_dir = session.working_directory.clone();
        let env_vars = session.env_vars.clone();

        // Build command
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd.exe");
            c.args(["/C", &command]);
            c
        } else {
            let mut c = Command::new("/bin/bash");
            c.args(["-c", &command]);
            c
        };

        cmd.current_dir(&working_dir);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        for (k, v) in &env_vars {
            cmd.env(k, v);
        }

        match cmd.spawn() {
            Ok(child) => {
                session.background_child = Some(child);
                session.background_running = true;

                Ok(ToolResult::success(format!(
                    "Background command started: {}. Use read_terminal_output to check progress.",
                    command
                )))
            }
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to start background command: {}",
                e
            ))),
        }
    }
}

/// Read accumulated output from a background command.
pub struct ReadTerminalOutputTool;

#[async_trait]
impl Tool for ReadTerminalOutputTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_terminal_output".to_string(),
            description: "Read output from a running or completed background command. Reports if the command is still running.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(&self, _params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let mut session = ctx.terminal_session.lock().await;

        if session.background_child.is_none() {
            return Ok(ToolResult::error(
                "No background command is running. Use execute_command_background first.",
            ));
        }

        // Try to read available output
        let mut new_stdout = String::new();
        let mut new_stderr = String::new();

        if let Some(ref mut child) = session.background_child {
            // Check if process has exited
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process finished - read remaining output
                    if let Some(ref mut stdout) = child.stdout {
                        let mut buf = Vec::new();
                        let _ = stdout.read_to_end(&mut buf).await;
                        new_stdout = String::from_utf8_lossy(&buf).to_string();
                    }
                    if let Some(ref mut stderr) = child.stderr {
                        let mut buf = Vec::new();
                        let _ = stderr.read_to_end(&mut buf).await;
                        new_stderr = String::from_utf8_lossy(&buf).to_string();
                    }

                    session.background_running = false;
                    session.background_output.push_str(&new_stdout);
                    session.background_output.push_str(&new_stderr);

                    let exit_code = status.code().unwrap_or(-1);
                    let output = session.background_output.clone();

                    // Truncate if too long
                    let output_display = if output.len() > 50_000 {
                        format!(
                            "{}...\n[Truncated, {} total chars]",
                            &output[..50_000],
                            output.len()
                        )
                    } else {
                        output
                    };

                    return Ok(ToolResult::success(format!(
                        "Background command COMPLETED (exit code: {})\n\n--- output ---\n{}",
                        exit_code,
                        output_display.trim()
                    )));
                }
                Ok(None) => {
                    // Still running
                    return Ok(ToolResult::success(
                        "Background command is still running. Check again later with read_terminal_output.".to_string(),
                    ));
                }
                Err(e) => {
                    session.background_running = false;
                    return Ok(ToolResult::error(format!(
                        "Error checking background command status: {}",
                        e
                    )));
                }
            }
        }

        Ok(ToolResult::error("No background command available"))
    }
}
