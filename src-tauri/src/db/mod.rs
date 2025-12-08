mod models;
mod repository;

pub use models::*;
pub use repository::*;

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::fs;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

static DB_POOL: OnceLock<SqlitePool> = OnceLock::new();

pub async fn init(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Get app data directory
    let app_dir = app
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory");

    fs::create_dir_all(&app_dir)?;

    let db_path = app_dir.join("tasker.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    // Create connection pool
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    // Run migrations
    run_migrations(&pool).await?;

    // Store pool globally using OnceLock (thread-safe)
    DB_POOL.set(pool).expect("Database already initialized");

    Ok(())
}

pub fn get_pool() -> &'static SqlitePool {
    DB_POOL.get().expect("Database not initialized")
}

async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS workflows (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            steps_json TEXT NOT NULL DEFAULT '[]',
            variables_json TEXT NOT NULL DEFAULT '[]',
            metadata_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            synced_at TEXT,
            version INTEGER NOT NULL DEFAULT 1,
            is_deleted INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_workflows_updated ON workflows(updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_workflows_deleted ON workflows(is_deleted);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS workflow_assets (
            id TEXT PRIMARY KEY,
            workflow_id TEXT NOT NULL,
            step_id TEXT NOT NULL,
            asset_type TEXT NOT NULL,
            file_path TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_assets_workflow ON workflow_assets(workflow_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS app_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            llm_config_json TEXT NOT NULL DEFAULT '{}',
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS execution_history (
            id TEXT PRIMARY KEY,
            workflow_id TEXT NOT NULL,
            started_at TEXT NOT NULL,
            completed_at TEXT,
            status TEXT NOT NULL,
            steps_completed INTEGER NOT NULL DEFAULT 0,
            total_steps INTEGER NOT NULL,
            error_message TEXT,
            llm_provider TEXT,
            result_json TEXT,
            FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_execution_workflow ON execution_history(workflow_id);
        CREATE INDEX IF NOT EXISTS idx_execution_started ON execution_history(started_at DESC);
        "#,
    )
    .execute(pool)
    .await?;

    // Migration: Add task_description column if it doesn't exist
    let _ = sqlx::query("ALTER TABLE workflows ADD COLUMN task_description TEXT")
        .execute(pool)
        .await;

    // Migration: Add stop_when column if it doesn't exist
    let _ = sqlx::query("ALTER TABLE workflows ADD COLUMN stop_when TEXT")
        .execute(pool)
        .await;

    // Migration: Add max_steps column if it doesn't exist
    let _ = sqlx::query("ALTER TABLE workflows ADD COLUMN max_steps INTEGER")
        .execute(pool)
        .await;

    // Migration: Add default_max_steps column to app_settings if it doesn't exist
    let _ = sqlx::query("ALTER TABLE app_settings ADD COLUMN default_max_steps INTEGER DEFAULT 50")
        .execute(pool)
        .await;

    Ok(())
}
