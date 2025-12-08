use crate::db;
use crate::taskfile::{self, Taskfile, ValidationResult};
use serde::{Deserialize, Serialize};
use std::fs;
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub workflow_id: String,
    pub name: String,
    pub validation: ValidationResult,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportResult {
    pub yaml: String,
    pub filename: String,
}

/// Parse and validate a Taskfile YAML string
#[tauri::command]
pub async fn parse_taskfile(yaml_content: String) -> Result<Taskfile, String> {
    taskfile::parse_yaml(&yaml_content)
}

/// Validate a parsed Taskfile
#[tauri::command]
pub async fn validate_taskfile(taskfile: Taskfile) -> Result<ValidationResult, String> {
    Ok(taskfile::validate(&taskfile))
}

/// Import a Taskfile and create a new workflow
#[tauri::command]
pub async fn import_taskfile(yaml_content: String) -> Result<ImportResult, String> {
    // Parse the YAML
    let taskfile = taskfile::parse_yaml(&yaml_content)?;

    // Validate
    let validation = taskfile::validate(&taskfile);
    if !validation.valid {
        let errors: Vec<String> = validation
            .errors
            .iter()
            .map(|e| format!("{}: {}", e.path, e.message))
            .collect();
        return Err(format!("Validation failed: {}", errors.join(", ")));
    }

    // Convert to workflow
    let workflow_dto = taskfile::taskfile_to_workflow(&taskfile);

    // Create workflow in database
    let request = db::CreateWorkflowRequest {
        name: workflow_dto.name.clone(),
        steps: Some(workflow_dto.steps),
        variables: Some(workflow_dto.variables),
        metadata: Some(workflow_dto.metadata),
        task_description: workflow_dto.task_description.clone(),
        stop_when: workflow_dto.stop_when.clone(),
        max_steps: workflow_dto.max_steps,
    };

    let workflow = db::create_workflow(request)
        .await
        .map_err(|e| e.to_string())?;

    Ok(ImportResult {
        workflow_id: workflow.id,
        name: workflow.name,
        validation,
    })
}

/// Export a workflow as a Taskfile YAML string
#[tauri::command]
pub async fn export_taskfile(workflow_id: String) -> Result<ExportResult, String> {
    // Get workflow from database
    let workflow = db::get_workflow_by_id(&workflow_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Workflow not found".to_string())?;

    // Convert to WorkflowDto
    let workflow: db::WorkflowDto = workflow.into();

    // Convert to Taskfile
    let taskfile = taskfile::workflow_to_taskfile(&workflow);

    // Generate YAML
    let yaml = taskfile::to_yaml_pretty(&taskfile)?;
    let filename = taskfile::suggest_filename(&taskfile);

    Ok(ExportResult { yaml, filename })
}

/// Get a suggested filename for a taskfile export
#[tauri::command]
pub async fn suggest_taskfile_filename(workflow_id: String) -> Result<String, String> {
    let workflow = db::get_workflow_by_id(&workflow_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Workflow not found".to_string())?;

    let workflow: db::WorkflowDto = workflow.into();
    let taskfile = taskfile::workflow_to_taskfile(&workflow);
    Ok(taskfile::suggest_filename(&taskfile))
}

/// Save a workflow as a Taskfile using a file save dialog
#[tauri::command]
pub async fn save_taskfile(app: tauri::AppHandle, workflow_id: String) -> Result<bool, String> {
    // Get workflow from database
    let workflow = db::get_workflow_by_id(&workflow_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Workflow not found".to_string())?;

    // Convert to Taskfile
    let workflow_dto: db::WorkflowDto = workflow.into();
    let taskfile_data = taskfile::workflow_to_taskfile(&workflow_dto);
    let yaml = taskfile::to_yaml_pretty(&taskfile_data)?;
    let filename = taskfile::suggest_filename(&taskfile_data);

    // Show save dialog
    let file_path = app
        .dialog()
        .file()
        .set_file_name(&filename)
        .add_filter("Taskfile", &["yaml", "taskfile.yaml"])
        .blocking_save_file();

    match file_path {
        Some(path) => {
            fs::write(path.as_path().unwrap(), yaml)
                .map_err(|e| format!("Failed to write file: {}", e))?;
            Ok(true)
        }
        None => Ok(false), // User cancelled
    }
}
