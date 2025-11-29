use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub steps_json: String,
    pub variables_json: String,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
    pub synced_at: Option<String>,
    pub version: i32,
    pub is_deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    #[serde(default)]
    pub order: i32,
    #[serde(default = "default_step_name")]
    pub name: String,
    pub action: serde_json::Value,
    pub description: Option<String>,
    pub screenshot_path: Option<String>,
    pub dom_snapshot: Option<serde_json::Value>,
}

fn default_step_name() -> String {
    "Step".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVariable {
    pub name: String,
    #[serde(rename = "type")]
    pub var_type: String,
    pub default_value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMetadata {
    pub start_url: Option<String>,
    pub llm_provider: Option<String>,
    pub recording_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkflowRequest {
    pub name: String,
    pub description: Option<String>,
    pub steps: Option<Vec<WorkflowStep>>,
    pub variables: Option<Vec<WorkflowVariable>>,
    pub metadata: Option<WorkflowMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWorkflowRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub steps: Option<Vec<WorkflowStep>>,
    pub variables: Option<Vec<WorkflowVariable>>,
    pub metadata: Option<WorkflowMetadata>,
}

// Frontend-friendly workflow representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<WorkflowStep>,
    pub variables: Vec<WorkflowVariable>,
    pub metadata: WorkflowMetadata,
    pub created_at: String,
    pub updated_at: String,
    pub version: i32,
}

impl From<Workflow> for WorkflowDto {
    fn from(w: Workflow) -> Self {
        WorkflowDto {
            id: w.id,
            name: w.name,
            description: w.description,
            steps: serde_json::from_str(&w.steps_json).unwrap_or_default(),
            variables: serde_json::from_str(&w.variables_json).unwrap_or_default(),
            metadata: serde_json::from_str(&w.metadata_json).unwrap_or(WorkflowMetadata {
                start_url: None,
                llm_provider: None,
                recording_source: "manual".to_string(),
            }),
            created_at: w.created_at,
            updated_at: w.updated_at,
            version: w.version,
        }
    }
}

// Settings models
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiKeys {
    pub gemini: Option<String>,
    pub openai: Option<String>,
    pub anthropic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub api_keys: ApiKeys,
    pub default_provider: String,
    pub default_model: String,
}

impl Default for LLMConfig {
    fn default() -> Self {
        LLMConfig {
            api_keys: ApiKeys::default(),
            default_provider: "gemini".to_string(),
            default_model: "gemini-2.0-flash".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub llm_config: LLMConfig,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            llm_config: LLMConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettingsRequest {
    pub api_keys: Option<ApiKeys>,
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
}
