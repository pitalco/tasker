use tokio::sync::broadcast;

use super::models::{LogLevel, RunLog, RunStep, RunStatus};
use super::repository::RunRepository;

/// WebSocket events for run updates
#[derive(Debug, Clone)]
pub enum RunEvent {
    /// A new step was executed
    Step {
        run_id: String,
        step: RunStep,
    },
    /// A new log was added
    Log {
        run_id: String,
        log: RunLog,
    },
    /// Run status changed
    Status {
        run_id: String,
        status: RunStatus,
        error: Option<String>,
    },
}

/// Run logger for structured logging with persistence and broadcast
pub struct RunLogger {
    repository: RunRepository,
    broadcast: broadcast::Sender<RunEvent>,
}

impl RunLogger {
    /// Create a new run logger
    pub fn new(repository: RunRepository) -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            repository,
            broadcast: tx,
        }
    }

    /// Subscribe to run events
    pub fn subscribe(&self) -> broadcast::Receiver<RunEvent> {
        self.broadcast.subscribe()
    }

    /// Log a debug message
    pub fn debug(&self, run_id: &str, message: impl Into<String>) {
        self.log(run_id, LogLevel::Debug, message.into(), None);
    }

    /// Log an info message
    pub fn info(&self, run_id: &str, message: impl Into<String>) {
        self.log(run_id, LogLevel::Info, message.into(), None);
    }

    /// Log a warning message
    pub fn warn(&self, run_id: &str, message: impl Into<String>) {
        self.log(run_id, LogLevel::Warn, message.into(), None);
    }

    /// Log an error message
    pub fn error(&self, run_id: &str, message: impl Into<String>) {
        self.log(run_id, LogLevel::Error, message.into(), None);
    }

    /// Log a message with metadata
    pub fn log_with_metadata(
        &self,
        run_id: &str,
        level: LogLevel,
        message: impl Into<String>,
        metadata: serde_json::Value,
    ) {
        self.log(run_id, level, message.into(), Some(metadata));
    }

    /// Internal log method
    fn log(
        &self,
        run_id: &str,
        level: LogLevel,
        message: String,
        metadata: Option<serde_json::Value>,
    ) {
        let log = RunLog::new(run_id.to_string(), level, message);
        let log = if let Some(meta) = metadata {
            log.with_metadata(meta)
        } else {
            log
        };

        // Persist to database
        if let Err(e) = self.repository.create_log(&log) {
            tracing::error!("Failed to persist log: {}", e);
        }

        // Broadcast to WebSocket clients
        let _ = self.broadcast.send(RunEvent::Log {
            run_id: run_id.to_string(),
            log,
        });
    }

    /// Record a step execution
    pub fn step(&self, step: &RunStep) {
        // Persist to database
        if let Err(e) = self.repository.create_step(step) {
            tracing::error!("Failed to persist step: {}", e);
        }

        // Broadcast to WebSocket clients
        let _ = self.broadcast.send(RunEvent::Step {
            run_id: step.run_id.clone(),
            step: step.clone(),
        });
    }

    /// Update a step after execution
    pub fn update_step(&self, step: &RunStep) {
        // Update in database
        if let Err(e) = self.repository.update_step(step) {
            tracing::error!("Failed to update step: {}", e);
        }

        // Broadcast updated step to WebSocket clients
        let _ = self.broadcast.send(RunEvent::Step {
            run_id: step.run_id.clone(),
            step: step.clone(),
        });
    }

    /// Update run status
    pub fn status(&self, run_id: &str, status: RunStatus, error: Option<String>) {
        // Update in database
        if let Err(e) = self.repository.update_run_status(run_id, status, error.as_deref()) {
            tracing::error!("Failed to update run status: {}", e);
        }

        // Broadcast to WebSocket clients
        let _ = self.broadcast.send(RunEvent::Status {
            run_id: run_id.to_string(),
            status,
            error,
        });
    }

    /// Get the repository (for direct access if needed)
    pub fn repository(&self) -> &RunRepository {
        &self.repository
    }
}

impl Clone for RunLogger {
    fn clone(&self) -> Self {
        Self {
            repository: self.repository.clone(),
            broadcast: self.broadcast.clone(),
        }
    }
}
