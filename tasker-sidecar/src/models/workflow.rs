use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Click,
    Type,
    Navigate,
    Scroll,
    Hover,
    Select,
    Wait,
    Screenshot,
    Extract,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SelectorStrategy {
    Css,
    Xpath,
    Text,
    AriaLabel,
    TestId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementSelector {
    pub strategy: SelectorStrategy,
    pub value: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallback_selectors: Vec<FallbackSelector>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackSelector {
    pub strategy: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coordinates {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserAction {
    #[serde(alias = "type")]
    pub action_type: ActionType,
    #[serde(default, deserialize_with = "flexible_selector", skip_serializing_if = "Option::is_none")]
    pub selector: Option<ElementSelector>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(alias = "text", skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<Coordinates>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub options: HashMap<String, serde_json::Value>,
    // Extra fields from Tauri format
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clear_first: Option<bool>,
}

/// Deserialize selector from either ElementSelector format or Tauri's {css, xpath, text, aria_label} format
fn flexible_selector<'de, D>(deserializer: D) -> Result<Option<ElementSelector>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum FlexibleSelector {
        // Proper ElementSelector format
        Proper(ElementSelector),
        // Tauri format: {css?, xpath?, text?, aria_label?}
        Tauri {
            css: Option<String>,
            xpath: Option<String>,
            text: Option<String>,
            aria_label: Option<String>,
        },
    }

    let value: Option<FlexibleSelector> = Option::deserialize(deserializer)?;

    Ok(match value {
        Some(FlexibleSelector::Proper(sel)) => Some(sel),
        Some(FlexibleSelector::Tauri { css, xpath, text, aria_label }) => {
            // Convert Tauri format to ElementSelector, preferring css > xpath > text > aria_label
            if let Some(css_val) = css {
                Some(ElementSelector {
                    strategy: SelectorStrategy::Css,
                    value: css_val,
                    fallback_selectors: vec![],
                })
            } else if let Some(xpath_val) = xpath {
                Some(ElementSelector {
                    strategy: SelectorStrategy::Xpath,
                    value: xpath_val,
                    fallback_selectors: vec![],
                })
            } else if let Some(text_val) = text {
                Some(ElementSelector {
                    strategy: SelectorStrategy::Text,
                    value: text_val,
                    fallback_selectors: vec![],
                })
            } else {
                aria_label.map(|aria_val| ElementSelector {
                    strategy: SelectorStrategy::AriaLabel,
                    value: aria_val,
                    fallback_selectors: vec![],
                })
            }
        }
        None => None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    #[serde(default)]
    pub order: i32,
    #[serde(default = "default_step_name")]
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(deserialize_with = "flexible_action")]
    pub action: BrowserAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dom_snapshot: Option<String>,
    #[serde(default = "default_wait_after")]
    pub wait_after_ms: i32,
    #[serde(default = "default_retry_count")]
    pub retry_count: i32,
    #[serde(default = "default_timeout")]
    pub timeout_ms: i32,
}

/// Deserialize action from either BrowserAction format or Tauri's JSON format
fn flexible_action<'de, D>(deserializer: D) -> Result<BrowserAction, D::Error>
where
    D: Deserializer<'de>,
{
    // First try to deserialize as a generic Value
    let value: serde_json::Value = serde_json::Value::deserialize(deserializer)?;

    // Try to deserialize directly as BrowserAction
    serde_json::from_value(value).map_err(serde::de::Error::custom)
}

fn default_step_name() -> String {
    "Step".to_string()
}
fn default_wait_after() -> i32 {
    500
}
fn default_retry_count() -> i32 {
    3
}
fn default_timeout() -> i32 {
    30000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowMetadata {
    #[serde(default = "default_recording_source")]
    pub recording_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_viewport: Option<Viewport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    // Tauri compatibility fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_provider: Option<String>,
}

fn default_recording_source() -> String {
    "manual".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub start_url: String,
    #[serde(default)]
    pub steps: Vec<WorkflowStep>,
    #[serde(default, deserialize_with = "flexible_variables")]
    pub variables: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub metadata: WorkflowMetadata,
    #[serde(default = "default_datetime", deserialize_with = "flexible_datetime")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "default_datetime", deserialize_with = "flexible_datetime")]
    pub updated_at: DateTime<Utc>,
    // Extra fields from Tauri
    #[serde(default)]
    pub version: i32,
    /// Task description - what this workflow automates
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_description: Option<String>,
}

fn default_datetime() -> DateTime<Utc> {
    Utc::now()
}

/// Deserialize datetime from either DateTime<Utc> format or String format
fn flexible_datetime<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum FlexibleDateTime {
        DateTime(DateTime<Utc>),
        String(String),
    }

    match FlexibleDateTime::deserialize(deserializer)? {
        FlexibleDateTime::DateTime(dt) => Ok(dt),
        FlexibleDateTime::String(s) => {
            // Try parsing as ISO 8601
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .or_else(|_| {
                    // Try other common formats
                    chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
                        .map(|dt| dt.and_utc())
                })
                .unwrap_or_else(|_| Utc::now())
                .pipe(Ok)
        }
    }
}

// Helper trait for pipe operator
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}
impl<T> Pipe for T {}

/// Deserialize variables from either HashMap or Vec<{name, type, default_value}> format
fn flexible_variables<'de, D>(deserializer: D) -> Result<HashMap<String, serde_json::Value>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct TauriVariable {
        name: String,
        #[serde(rename = "type")]
        _var_type: Option<String>,
        default_value: Option<serde_json::Value>,
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum FlexibleVariables {
        Map(HashMap<String, serde_json::Value>),
        Array(Vec<TauriVariable>),
    }

    match FlexibleVariables::deserialize(deserializer)? {
        FlexibleVariables::Map(map) => Ok(map),
        FlexibleVariables::Array(arr) => {
            Ok(arr
                .into_iter()
                .map(|v| (v.name, v.default_value.unwrap_or(serde_json::Value::Null)))
                .collect())
        }
    }
}

impl Workflow {
    pub fn new(name: String, start_url: String) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            start_url,
            steps: Vec::new(),
            variables: HashMap::new(),
            metadata: WorkflowMetadata::default(),
            created_at: now,
            updated_at: now,
            version: 1,
            task_description: None,
        }
    }

    /// Create a text-only workflow from a task description
    /// The AI will figure out how to execute this based on the description
    pub fn from_description(name: String, task_description: String) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            start_url: String::new(), // AI extracts from description
            steps: Vec::new(),
            variables: HashMap::new(),
            metadata: WorkflowMetadata {
                recording_source: "text_description".to_string(),
                ..Default::default()
            },
            created_at: now,
            updated_at: now,
            version: 1,
            task_description: Some(task_description),
        }
    }

    /// Resolve start_url from metadata or first navigate step if not set at top level
    pub fn resolve_start_url(&mut self) {
        if self.start_url.is_empty() {
            // Try metadata first
            if let Some(url) = &self.metadata.start_url {
                self.start_url = url.clone();
                return;
            }
            // Fall back to first navigate step
            for step in &self.steps {
                if step.action.action_type == ActionType::Navigate {
                    if let Some(url) = &step.action.url {
                        self.start_url = url.clone();
                        return;
                    }
                }
            }
        }
    }
}
