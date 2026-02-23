use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::api::state::AppState;
use crate::browser::BrowserManager;
use crate::tools::{RunSpawner, RunSpawnerInfo, RunSpawnerStatus};

use super::models::Run;
use super::{ExecutorConfig, RunExecutor, RunLogger};

/// Maximum nesting depth for child agents
const MAX_DEPTH: u32 = 5;

/// RunSpawner implementation that uses AppState to spawn and track child runs.
pub struct AppStateRunSpawner {
    state: Arc<AppState>,
    /// LLM config inherited from the parent run
    model: String,
    api_key: Option<String>,
    provider: Option<String>,
    allowed_directories: Vec<std::path::PathBuf>,
}

impl AppStateRunSpawner {
    pub fn new(
        state: Arc<AppState>,
        model: String,
        api_key: Option<String>,
        provider: Option<String>,
        allowed_directories: Vec<std::path::PathBuf>,
    ) -> Self {
        Self {
            state,
            model,
            api_key,
            provider,
            allowed_directories,
        }
    }
}

#[async_trait]
impl RunSpawner for AppStateRunSpawner {
    async fn spawn_run(
        &self,
        task_description: String,
        workflow_id: Option<String>,
        variables: Option<HashMap<String, String>>,
        parent_depth: u32,
    ) -> Result<String> {
        let child_depth = parent_depth + 1;
        if child_depth > MAX_DEPTH {
            return Err(anyhow::anyhow!(
                "Maximum agent nesting depth ({}) exceeded",
                MAX_DEPTH
            ));
        }

        let repo = self
            .state
            .runs_repository
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Runs repository not initialized"))?;

        // Create child run
        let mut run = Run::new(
            workflow_id.clone(),
            None,
            Some(task_description.clone()),
            None,
        );

        // Add variables to metadata if provided
        if let Some(vars) = &variables {
            let vars_value = serde_json::to_value(vars).unwrap_or_default();
            run.metadata = serde_json::json!({
                "variables": vars_value,
                "parent_spawned": true,
                "agent_depth": child_depth,
            });
        } else {
            run.metadata = serde_json::json!({
                "parent_spawned": true,
                "agent_depth": child_depth,
            });
        }

        let run_id = run.id.clone();

        // Persist to DB
        repo.create_run(&run)?;

        // Track in active runs
        self.state.active_runs.insert(run_id.clone(), run.clone());

        // Create a headless browser for the child
        let browser = Arc::new(BrowserManager::new());
        browser
            .launch_incognito("about:blank", true, None)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to launch browser for child agent: {}", e))?;

        // Create executor with inherited config
        let logger = RunLogger::new(repo.clone());
        let config = ExecutorConfig {
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            max_steps: 50,
            headless: true,
            provider: self.provider.clone(),
            min_llm_delay_ms: 2000,
            capture_screenshots: false, // Headless child, no need for screenshots in most cases
            allowed_directories: self.allowed_directories.clone(),
            run_spawner: Some(Arc::new(AppStateRunSpawner::new(
                Arc::clone(&self.state),
                self.model.clone(),
                self.api_key.clone(),
                self.provider.clone(),
                self.allowed_directories.clone(),
            ))),
            agent_depth: child_depth,
            command_timeout_secs: 30,
        };

        let executor = RunExecutor::new(logger, Arc::clone(&browser), config);
        let cancel_token = executor.cancel_token();

        // Store cancel token
        self.state
            .active_executors
            .insert(run_id.clone(), cancel_token);

        // Spawn execution in background
        let run_for_exec = run.clone();
        let browser_for_cleanup = Arc::clone(&browser);
        let state_for_cleanup = Arc::clone(&self.state);
        let run_id_for_cleanup = run_id.clone();

        tokio::spawn(async move {
            let result = executor.execute(&run_for_exec).await;

            match &result {
                Ok(()) => tracing::info!("Child run {} finished", run_id_for_cleanup),
                Err(e) => tracing::warn!("Child run {} failed: {}", run_id_for_cleanup, e),
            }

            // Clean up
            let _ = browser_for_cleanup.close().await;
            state_for_cleanup
                .active_runs
                .remove(&run_id_for_cleanup);
            state_for_cleanup
                .active_executors
                .remove(&run_id_for_cleanup);
        });

        tracing::info!(
            "Spawned child agent run {} (depth {}) for task: {}",
            run_id,
            child_depth,
            task_description
        );

        Ok(run_id)
    }

    async fn get_run_status(&self, run_id: &str) -> Result<RunSpawnerStatus> {
        // Check active runs first
        if let Some(run) = self.state.active_runs.get(run_id) {
            return Ok(RunSpawnerStatus {
                run_id: run_id.to_string(),
                status: run.status.as_str().to_string(),
                result: run.result.clone(),
                error: run.error.clone(),
            });
        }

        // Fall back to DB
        let repo = self
            .state
            .runs_repository
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Runs repository not initialized"))?;

        let run = repo
            .get_run(run_id)?
            .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;

        Ok(RunSpawnerStatus {
            run_id: run_id.to_string(),
            status: run.status.as_str().to_string(),
            result: run.result,
            error: run.error,
        })
    }

    async fn list_active_runs(&self) -> Result<Vec<RunSpawnerInfo>> {
        let mut runs = Vec::new();
        for entry in self.state.active_runs.iter() {
            runs.push(RunSpawnerInfo {
                run_id: entry.key().clone(),
                task_description: entry.value().task_description.clone(),
                status: entry.value().status.as_str().to_string(),
            });
        }
        Ok(runs)
    }
}
