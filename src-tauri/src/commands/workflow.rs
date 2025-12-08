use crate::db::{self, CreateWorkflowRequest, UpdateWorkflowRequest, WorkflowDto};

#[tauri::command]
pub async fn get_workflows() -> Result<Vec<WorkflowDto>, String> {
    let workflows = db::get_all_workflows().await.map_err(|e| e.to_string())?;

    Ok(workflows.into_iter().map(WorkflowDto::from).collect())
}

#[tauri::command]
pub async fn get_workflow(id: String) -> Result<Option<WorkflowDto>, String> {
    let workflow = db::get_workflow_by_id(&id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(workflow.map(WorkflowDto::from))
}

#[tauri::command]
pub async fn create_workflow(request: CreateWorkflowRequest) -> Result<WorkflowDto, String> {
    let workflow = db::create_workflow(request)
        .await
        .map_err(|e| e.to_string())?;

    Ok(WorkflowDto::from(workflow))
}

#[tauri::command]
pub async fn update_workflow(
    id: String,
    request: UpdateWorkflowRequest,
) -> Result<Option<WorkflowDto>, String> {
    let workflow = db::update_workflow(&id, request)
        .await
        .map_err(|e| e.to_string())?;

    Ok(workflow.map(WorkflowDto::from))
}

#[tauri::command]
pub async fn delete_workflow(id: String) -> Result<bool, String> {
    db::delete_workflow(&id).await.map_err(|e| e.to_string())
}
