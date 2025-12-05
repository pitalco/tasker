use super::models::*;
use super::get_pool;
use sqlx::Row;
use uuid::Uuid;
use chrono::Utc;

pub async fn get_all_workflows() -> Result<Vec<Workflow>, sqlx::Error> {
    let pool = get_pool();

    let rows = sqlx::query(
        r#"
        SELECT id, name, steps_json, variables_json, metadata_json,
               created_at, updated_at, synced_at, version, is_deleted, task_description,
               stop_when, max_steps
        FROM workflows
        WHERE is_deleted = 0
        ORDER BY updated_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let workflows: Vec<Workflow> = rows
        .iter()
        .map(|row| Workflow {
            id: row.get("id"),
            name: row.get("name"),
            steps_json: row.get("steps_json"),
            variables_json: row.get("variables_json"),
            metadata_json: row.get("metadata_json"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            synced_at: row.get("synced_at"),
            version: row.get("version"),
            is_deleted: row.get::<i32, _>("is_deleted") != 0,
            task_description: row.get("task_description"),
            stop_when: row.get("stop_when"),
            max_steps: row.get("max_steps"),
        })
        .collect();

    Ok(workflows)
}

pub async fn get_workflow_by_id(id: &str) -> Result<Option<Workflow>, sqlx::Error> {
    let pool = get_pool();

    let row = sqlx::query(
        r#"
        SELECT id, name, steps_json, variables_json, metadata_json,
               created_at, updated_at, synced_at, version, is_deleted, task_description,
               stop_when, max_steps
        FROM workflows
        WHERE id = ? AND is_deleted = 0
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| Workflow {
        id: r.get("id"),
        name: r.get("name"),
        steps_json: r.get("steps_json"),
        variables_json: r.get("variables_json"),
        metadata_json: r.get("metadata_json"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
        synced_at: r.get("synced_at"),
        version: r.get("version"),
        is_deleted: r.get::<i32, _>("is_deleted") != 0,
        task_description: r.get("task_description"),
        stop_when: r.get("stop_when"),
        max_steps: r.get("max_steps"),
    }))
}

pub async fn create_workflow(req: CreateWorkflowRequest) -> Result<Workflow, sqlx::Error> {
    let pool = get_pool();
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let steps_json = serde_json::to_string(&req.steps.unwrap_or_default()).unwrap();
    let variables_json = serde_json::to_string(&req.variables.unwrap_or_default()).unwrap();
    let metadata_json = serde_json::to_string(&req.metadata.unwrap_or(WorkflowMetadata {
        start_url: None,
        llm_provider: None,
        recording_source: "manual".to_string(),
    })).unwrap();

    sqlx::query(
        r#"
        INSERT INTO workflows (id, name, steps_json, variables_json, metadata_json, created_at, updated_at, version, task_description, stop_when, max_steps)
        VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&steps_json)
    .bind(&variables_json)
    .bind(&metadata_json)
    .bind(&now)
    .bind(&now)
    .bind(&req.task_description)
    .bind(&req.stop_when)
    .bind(&req.max_steps)
    .execute(pool)
    .await?;

    Ok(Workflow {
        id,
        name: req.name,
        steps_json,
        variables_json,
        metadata_json,
        created_at: now.clone(),
        updated_at: now,
        synced_at: None,
        version: 1,
        is_deleted: false,
        task_description: req.task_description,
        stop_when: req.stop_when,
        max_steps: req.max_steps,
    })
}

pub async fn update_workflow(id: &str, req: UpdateWorkflowRequest) -> Result<Option<Workflow>, sqlx::Error> {
    let pool = get_pool();

    // Get existing workflow
    let existing = get_workflow_by_id(id).await?;
    let Some(mut workflow) = existing else {
        return Ok(None);
    };

    let now = Utc::now().to_rfc3339();

    if let Some(name) = req.name {
        workflow.name = name;
    }
    if let Some(task_desc) = req.task_description {
        workflow.task_description = Some(task_desc);
    }
    if let Some(steps) = req.steps {
        workflow.steps_json = serde_json::to_string(&steps).unwrap();
    }
    if let Some(variables) = req.variables {
        workflow.variables_json = serde_json::to_string(&variables).unwrap();
    }
    if let Some(metadata) = req.metadata {
        workflow.metadata_json = serde_json::to_string(&metadata).unwrap();
    }
    if let Some(stop_when) = req.stop_when {
        workflow.stop_when = Some(stop_when);
    }
    if let Some(max_steps) = req.max_steps {
        workflow.max_steps = Some(max_steps);
    }

    workflow.updated_at = now;
    workflow.version += 1;

    sqlx::query(
        r#"
        UPDATE workflows
        SET name = ?, task_description = ?, steps_json = ?, variables_json = ?, metadata_json = ?,
            updated_at = ?, version = ?, stop_when = ?, max_steps = ?
        WHERE id = ?
        "#,
    )
    .bind(&workflow.name)
    .bind(&workflow.task_description)
    .bind(&workflow.steps_json)
    .bind(&workflow.variables_json)
    .bind(&workflow.metadata_json)
    .bind(&workflow.updated_at)
    .bind(&workflow.version)
    .bind(&workflow.stop_when)
    .bind(&workflow.max_steps)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(Some(workflow))
}

pub async fn delete_workflow(id: &str) -> Result<bool, sqlx::Error> {
    let pool = get_pool();
    let now = Utc::now().to_rfc3339();

    let result = sqlx::query(
        r#"
        UPDATE workflows
        SET is_deleted = 1, updated_at = ?
        WHERE id = ? AND is_deleted = 0
        "#,
    )
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

// Settings repository functions
pub async fn get_settings() -> Result<AppSettings, sqlx::Error> {
    let pool = get_pool();

    let row = sqlx::query(
        r#"
        SELECT llm_config_json, default_max_steps
        FROM app_settings
        WHERE id = 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let llm_config_json: String = r.get("llm_config_json");
            let llm_config: LLMConfig = serde_json::from_str(&llm_config_json)
                .unwrap_or_default();
            let default_max_steps: Option<i32> = r.get("default_max_steps");

            Ok(AppSettings {
                llm_config,
                default_max_steps: default_max_steps.unwrap_or(50),
            })
        }
        None => Ok(AppSettings::default()),
    }
}

pub async fn update_settings(req: UpdateSettingsRequest) -> Result<AppSettings, sqlx::Error> {
    let pool = get_pool();
    let now = Utc::now().to_rfc3339();

    // Get existing settings
    let mut settings = get_settings().await?;

    // Apply updates
    if let Some(api_keys) = req.api_keys {
        // Merge API keys (only update non-None values)
        if api_keys.gemini.is_some() {
            settings.llm_config.api_keys.gemini = api_keys.gemini;
        }
        if api_keys.openai.is_some() {
            settings.llm_config.api_keys.openai = api_keys.openai;
        }
        if api_keys.anthropic.is_some() {
            settings.llm_config.api_keys.anthropic = api_keys.anthropic;
        }
    }
    if let Some(default_provider) = req.default_provider {
        settings.llm_config.default_provider = default_provider;
    }
    if let Some(default_model) = req.default_model {
        settings.llm_config.default_model = default_model;
    }
    if let Some(default_max_steps) = req.default_max_steps {
        settings.default_max_steps = default_max_steps;
    }

    let llm_config_json = serde_json::to_string(&settings.llm_config).unwrap();

    // Upsert settings (SQLite UPSERT)
    sqlx::query(
        r#"
        INSERT INTO app_settings (id, llm_config_json, default_max_steps, updated_at)
        VALUES (1, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            llm_config_json = excluded.llm_config_json,
            default_max_steps = excluded.default_max_steps,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&llm_config_json)
    .bind(&settings.default_max_steps)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(settings)
}
