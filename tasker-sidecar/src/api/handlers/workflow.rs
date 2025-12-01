//! Workflow management handlers

use axum::{http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::models::Workflow;

/// Request to create a text-only workflow
#[derive(Debug, Deserialize)]
pub struct CreateWorkflowRequest {
    /// Name of the workflow
    pub name: String,
    /// Task description - natural language description of what to automate
    /// e.g. "Go to amazon.com and search for mechanical keyboards, then sort by price"
    pub task_description: String,
}

/// Response for workflow creation
#[derive(Debug, Serialize)]
pub struct CreateWorkflowResponse {
    pub workflow: Workflow,
}

/// Create a new text-only workflow
///
/// This creates a workflow with just a task description and no recorded steps.
/// When replayed, the AI agent will figure out how to execute based on the description.
pub async fn create_workflow(
    Json(request): Json<CreateWorkflowRequest>,
) -> Result<Json<CreateWorkflowResponse>, (StatusCode, String)> {
    // Validate inputs
    if request.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Workflow name cannot be empty".to_string(),
        ));
    }

    if request.task_description.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Task description cannot be empty".to_string(),
        ));
    }

    // Create the workflow from description
    let workflow = Workflow::from_description(
        request.name.trim().to_string(),
        request.task_description.trim().to_string(),
    );

    tracing::info!(
        "Created text-only workflow: {} ({})",
        workflow.name,
        workflow.id
    );

    Ok(Json(CreateWorkflowResponse { workflow }))
}
