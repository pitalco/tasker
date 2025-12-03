use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::models::{Run, RunListQuery, RunLog, RunStatus, RunStep};

/// Database path for runs (uses same Tauri data directory)
fn get_db_path() -> Result<PathBuf> {
    let data_dir = dirs::data_dir().ok_or_else(|| anyhow!("Could not find data directory"))?;
    let db_path = data_dir.join("com.tasker.app").join("runs.db");

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    Ok(db_path)
}

/// Run repository for SQLite persistence
pub struct RunRepository {
    conn: Arc<Mutex<Connection>>,
}

impl RunRepository {
    /// Create a new repository and initialize the schema
    pub fn new() -> Result<Self> {
        let db_path = get_db_path()?;
        let conn = Connection::open(&db_path)?;

        let repo = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        repo.init_schema()?;
        repo.run_migrations()?;
        Ok(repo)
    }

    /// Run any necessary migrations
    fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        // Add result column if it doesn't exist (migration for existing DBs)
        // SQLite doesn't have IF NOT EXISTS for ALTER TABLE, so we check manually
        let has_result_column: bool = conn
            .prepare("SELECT result FROM runs LIMIT 0")
            .is_ok();

        if !has_result_column {
            conn.execute("ALTER TABLE runs ADD COLUMN result TEXT", [])?;
        }

        Ok(())
    }

    /// Initialize the database schema
    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        conn.execute_batch(
            r#"
            -- Runs table
            CREATE TABLE IF NOT EXISTS runs (
                id TEXT PRIMARY KEY,
                workflow_id TEXT,
                workflow_name TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                task_description TEXT,
                custom_instructions TEXT,
                started_at TEXT NOT NULL,
                completed_at TEXT,
                error TEXT,
                result TEXT,
                metadata TEXT DEFAULT '{}'
            );

            CREATE INDEX IF NOT EXISTS idx_runs_workflow_id ON runs(workflow_id);
            CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
            CREATE INDEX IF NOT EXISTS idx_runs_started_at ON runs(started_at DESC);

            -- Run steps table
            CREATE TABLE IF NOT EXISTS run_steps (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                step_number INTEGER NOT NULL,
                tool_name TEXT NOT NULL,
                params TEXT NOT NULL DEFAULT '{}',
                result TEXT,
                success INTEGER NOT NULL DEFAULT 0,
                error TEXT,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                timestamp TEXT NOT NULL,
                screenshot TEXT,
                FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_run_steps_run_id ON run_steps(run_id);
            CREATE INDEX IF NOT EXISTS idx_run_steps_timestamp ON run_steps(timestamp);

            -- Run logs table
            CREATE TABLE IF NOT EXISTS run_logs (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                level TEXT NOT NULL DEFAULT 'info',
                message TEXT NOT NULL,
                metadata TEXT,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_run_logs_run_id ON run_logs(run_id);
            CREATE INDEX IF NOT EXISTS idx_run_logs_timestamp ON run_logs(timestamp);
            "#,
        )?;

        Ok(())
    }

    /// Create a new run
    pub fn create_run(&self, run: &Run) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        conn.execute(
            r#"
            INSERT INTO runs (id, workflow_id, workflow_name, status, task_description,
                              custom_instructions, started_at, completed_at, error, result, metadata)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                run.id,
                run.workflow_id,
                run.workflow_name,
                run.status.as_str(),
                run.task_description,
                run.custom_instructions,
                run.started_at.to_rfc3339(),
                run.completed_at.map(|dt| dt.to_rfc3339()),
                run.error,
                run.result,
                serde_json::to_string(&run.metadata)?,
            ],
        )?;

        Ok(())
    }

    /// Get a run by ID
    pub fn get_run(&self, id: &str) -> Result<Option<Run>> {
        let conn = self.conn.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id, workflow_id, workflow_name, status, task_description,
                   custom_instructions, started_at, completed_at, error, result, metadata
            FROM runs WHERE id = ?1
            "#,
        )?;

        let run = stmt.query_row(params![id], |row| {
            Ok(self.row_to_run(row))
        }).optional()?;

        match run {
            Some(Ok(mut run)) => {
                // Fetch steps and logs
                drop(stmt);
                run.steps = self.get_steps_for_run_internal(&conn, &run.id)?;
                run.logs = self.get_logs_for_run_internal(&conn, &run.id)?;
                Ok(Some(run))
            }
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    /// Update a run's status
    pub fn update_run_status(&self, id: &str, status: RunStatus, error: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        let completed_at = if status == RunStatus::Completed || status == RunStatus::Failed || status == RunStatus::Cancelled {
            Some(chrono::Utc::now().to_rfc3339())
        } else {
            None
        };

        conn.execute(
            r#"
            UPDATE runs SET status = ?1, error = ?2, completed_at = COALESCE(?3, completed_at)
            WHERE id = ?4
            "#,
            params![status.as_str(), error, completed_at, id],
        )?;

        Ok(())
    }

    /// List runs with optional filters
    pub fn list_runs(&self, query: &RunListQuery) -> Result<(Vec<Run>, i64)> {
        let conn = self.conn.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        // Build WHERE clause
        let mut conditions = Vec::new();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(workflow_id) = &query.workflow_id {
            conditions.push(format!("workflow_id = ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(workflow_id.clone()));
        }

        if let Some(status) = &query.status {
            conditions.push(format!("status = ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(status.as_str().to_string()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Get total count
        let count_sql = format!("SELECT COUNT(*) FROM runs {}", where_clause);
        let total: i64 = {
            let mut stmt = conn.prepare(&count_sql)?;
            let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
            stmt.query_row(params_refs.as_slice(), |row| row.get(0))?
        };

        // Get paginated results
        let order = if query.sort_desc { "DESC" } else { "ASC" };
        let offset = (query.page - 1) * query.per_page;

        let select_sql = format!(
            r#"
            SELECT id, workflow_id, workflow_name, status, task_description,
                   custom_instructions, started_at, completed_at, error, result, metadata
            FROM runs {}
            ORDER BY started_at {}
            LIMIT ?{} OFFSET ?{}
            "#,
            where_clause,
            order,
            params_vec.len() + 1,
            params_vec.len() + 2
        );

        params_vec.push(Box::new(query.per_page));
        params_vec.push(Box::new(offset));

        let mut stmt = conn.prepare(&select_sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let runs: Vec<Run> = stmt
            .query_map(params_refs.as_slice(), |row| Ok(self.row_to_run(row)))?
            .filter_map(|r| r.ok())
            .filter_map(|r| r.ok())
            .collect();

        Ok((runs, total))
    }

    /// Delete a run and its associated steps and logs
    pub fn delete_run(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        // Delete logs first (foreign key)
        conn.execute("DELETE FROM run_logs WHERE run_id = ?1", params![id])?;

        // Delete steps (foreign key)
        conn.execute("DELETE FROM run_steps WHERE run_id = ?1", params![id])?;

        // Delete run
        let deleted = conn.execute("DELETE FROM runs WHERE id = ?1", params![id])?;

        Ok(deleted > 0)
    }

    /// Create a run step
    pub fn create_step(&self, step: &RunStep) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        conn.execute(
            r#"
            INSERT INTO run_steps (id, run_id, step_number, tool_name, params, result,
                                   success, error, duration_ms, timestamp, screenshot)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                step.id,
                step.run_id,
                step.step_number,
                step.tool_name,
                serde_json::to_string(&step.params)?,
                step.result.as_ref().map(serde_json::to_string).transpose()?,
                step.success as i32,
                step.error,
                step.duration_ms,
                step.timestamp.to_rfc3339(),
                step.screenshot,
            ],
        )?;

        Ok(())
    }

    /// Update a run step (after execution)
    pub fn update_step(&self, step: &RunStep) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        conn.execute(
            r#"
            UPDATE run_steps SET result = ?1, success = ?2, error = ?3,
                                 duration_ms = ?4, screenshot = ?5
            WHERE id = ?6
            "#,
            params![
                step.result.as_ref().map(serde_json::to_string).transpose()?,
                step.success as i32,
                step.error,
                step.duration_ms,
                step.screenshot,
                step.id,
            ],
        )?;

        Ok(())
    }

    /// Get steps for a run
    pub fn get_steps_for_run(&self, run_id: &str) -> Result<Vec<RunStep>> {
        let conn = self.conn.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        self.get_steps_for_run_internal(&conn, run_id)
    }

    fn get_steps_for_run_internal(&self, conn: &Connection, run_id: &str) -> Result<Vec<RunStep>> {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, run_id, step_number, tool_name, params, result,
                   success, error, duration_ms, timestamp, screenshot
            FROM run_steps WHERE run_id = ?1
            ORDER BY step_number ASC
            "#,
        )?;

        let steps: Vec<RunStep> = stmt
            .query_map(params![run_id], |row| {
                Ok(RunStep {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    step_number: row.get(2)?,
                    tool_name: row.get(3)?,
                    params: {
                        let json_str: String = row.get(4)?;
                        serde_json::from_str(&json_str).unwrap_or_default()
                    },
                    result: {
                        let json_str: Option<String> = row.get(5)?;
                        json_str.and_then(|s| serde_json::from_str(&s).ok())
                    },
                    success: row.get::<_, i32>(6)? != 0,
                    error: row.get(7)?,
                    duration_ms: row.get(8)?,
                    timestamp: {
                        let ts: String = row.get(9)?;
                        chrono::DateTime::parse_from_rfc3339(&ts)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_else(|_| chrono::Utc::now())
                    },
                    screenshot: row.get(10)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(steps)
    }

    /// Create a run log
    pub fn create_log(&self, log: &RunLog) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        conn.execute(
            r#"
            INSERT INTO run_logs (id, run_id, level, message, metadata, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                log.id,
                log.run_id,
                log.level.as_str(),
                log.message,
                log.metadata.as_ref().map(serde_json::to_string).transpose()?,
                log.timestamp.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    /// Get logs for a run
    pub fn get_logs_for_run(&self, run_id: &str) -> Result<Vec<RunLog>> {
        let conn = self.conn.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        self.get_logs_for_run_internal(&conn, run_id)
    }

    fn get_logs_for_run_internal(&self, conn: &Connection, run_id: &str) -> Result<Vec<RunLog>> {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, run_id, level, message, metadata, timestamp
            FROM run_logs WHERE run_id = ?1
            ORDER BY timestamp ASC
            "#,
        )?;

        let logs: Vec<RunLog> = stmt
            .query_map(params![run_id], |row| {
                Ok(RunLog {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    level: {
                        let level_str: String = row.get(2)?;
                        level_str.parse().unwrap_or_default()
                    },
                    message: row.get(3)?,
                    metadata: {
                        let json_str: Option<String> = row.get(4)?;
                        json_str.and_then(|s| serde_json::from_str(&s).ok())
                    },
                    timestamp: {
                        let ts: String = row.get(5)?;
                        chrono::DateTime::parse_from_rfc3339(&ts)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_else(|_| chrono::Utc::now())
                    },
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(logs)
    }

    /// Helper to convert a row to a Run
    fn row_to_run(&self, row: &rusqlite::Row) -> Result<Run> {
        Ok(Run {
            id: row.get(0)?,
            workflow_id: row.get(1)?,
            workflow_name: row.get(2)?,
            status: {
                let status_str: String = row.get(3)?;
                status_str.parse().unwrap_or_default()
            },
            task_description: row.get(4)?,
            custom_instructions: row.get(5)?,
            started_at: {
                let ts: String = row.get(6)?;
                chrono::DateTime::parse_from_rfc3339(&ts)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now())
            },
            completed_at: {
                let ts: Option<String> = row.get(7)?;
                ts.and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .ok()
                })
            },
            error: row.get(8)?,
            result: row.get(9)?,
            metadata: {
                let json_str: String = row.get(10)?;
                serde_json::from_str(&json_str).unwrap_or_default()
            },
            steps: Vec::new(),
            logs: Vec::new(),
        })
    }

    /// Update a run's result
    pub fn update_run_result(&self, id: &str, result: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        conn.execute(
            "UPDATE runs SET result = ?1 WHERE id = ?2",
            params![result, id],
        )?;

        Ok(())
    }
}

impl Clone for RunRepository {
    fn clone(&self) -> Self {
        Self {
            conn: Arc::clone(&self.conn),
        }
    }
}

// Make RunRepository thread-safe
unsafe impl Send for RunRepository {}
unsafe impl Sync for RunRepository {}
