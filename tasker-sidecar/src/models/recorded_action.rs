use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A recorded action in tool-based format with hints for AI guidance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedAction {
    /// Order in the sequence
    pub order: i32,
    /// The tool name (e.g., "click_element", "input_text", "go_to_url")
    pub tool: String,
    /// Tool parameters
    pub params: Value,
    /// Additional hints to help AI locate the element
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hints: Option<ActionHints>,
    /// Screenshot captured after this action (base64)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
    /// Timestamp when action was recorded
    #[serde(default)]
    pub timestamp: i64,
}

/// Hints to help AI find the target element
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActionHints {
    /// CSS selector used during recording
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub css_selector: Option<String>,
    /// XPath selector
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xpath: Option<String>,
    /// Visible text of the element
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Placeholder or label text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Element tag name (button, input, a, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_name: Option<String>,
    /// ARIA label if available
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aria_label: Option<String>,
    /// Additional context description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Coordinates where the click occurred
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<(i32, i32)>,
}

impl RecordedAction {
    /// Create a navigate action
    pub fn navigate(order: i32, url: &str, timestamp: i64) -> Self {
        Self {
            order,
            tool: "go_to_url".to_string(),
            params: json!({ "url": url }),
            hints: Some(ActionHints {
                description: Some(format!("Navigate to {}", url)),
                ..Default::default()
            }),
            screenshot: None,
            timestamp,
        }
    }

    /// Create a click action
    pub fn click(
        order: i32,
        selector: Option<&str>,
        text: Option<&str>,
        coordinates: Option<(i32, i32)>,
        timestamp: i64,
    ) -> Self {
        Self {
            order,
            tool: "click_element".to_string(),
            // For recorded actions, we include selector hints rather than index
            // The AI will use these hints to find the right element
            params: json!({}),
            hints: Some(ActionHints {
                css_selector: selector.map(|s| s.to_string()),
                text: text.map(|t| t.to_string()),
                coordinates,
                description: text.map(|t| format!("Click element with text: {}", t)),
                ..Default::default()
            }),
            screenshot: None,
            timestamp,
        }
    }

    /// Create an input/type action
    pub fn input(
        order: i32,
        selector: Option<&str>,
        value: &str,
        placeholder: Option<&str>,
        timestamp: i64,
    ) -> Self {
        Self {
            order,
            tool: "input_text".to_string(),
            params: json!({ "text": value }),
            hints: Some(ActionHints {
                css_selector: selector.map(|s| s.to_string()),
                placeholder: placeholder.map(|p| p.to_string()),
                description: Some(format!("Type '{}' into input field", truncate(value, 30))),
                ..Default::default()
            }),
            screenshot: None,
            timestamp,
        }
    }

    /// Create a select/dropdown action
    pub fn select(
        order: i32,
        selector: Option<&str>,
        value: &str,
        timestamp: i64,
    ) -> Self {
        Self {
            order,
            tool: "select_dropdown_option".to_string(),
            params: json!({ "option": value }),
            hints: Some(ActionHints {
                css_selector: selector.map(|s| s.to_string()),
                description: Some(format!("Select option: {}", value)),
                ..Default::default()
            }),
            screenshot: None,
            timestamp,
        }
    }

    /// Create a scroll action
    pub fn scroll(order: i32, direction: &str, amount: i32, timestamp: i64) -> Self {
        let tool = if direction == "up" {
            "scroll_up"
        } else {
            "scroll_down"
        };
        Self {
            order,
            tool: tool.to_string(),
            params: json!({ "amount": amount }),
            hints: Some(ActionHints {
                description: Some(format!("Scroll {} by {} pixels", direction, amount)),
                ..Default::default()
            }),
            screenshot: None,
            timestamp,
        }
    }

    /// Create a keyboard action
    pub fn send_keys(order: i32, keys: &str, timestamp: i64) -> Self {
        Self {
            order,
            tool: "send_keys".to_string(),
            params: json!({ "keys": keys }),
            hints: Some(ActionHints {
                description: Some(format!("Press key: {}", keys)),
                ..Default::default()
            }),
            screenshot: None,
            timestamp,
        }
    }

    /// Format as human-readable hint for AI prompt
    pub fn to_hint_string(&self) -> String {
        let mut parts = vec![format!("{}. {} ", self.order, self.tool)];

        if let Some(hints) = &self.hints {
            if let Some(desc) = &hints.description {
                parts.push(format!("- {}", desc));
            }
            if let Some(sel) = &hints.css_selector {
                parts.push(format!(" [selector: {}]", sel));
            }
            if let Some(text) = &hints.text {
                parts.push(format!(" [text: '{}']", truncate(text, 50)));
            }
        }

        // Include params if they have meaningful values
        if let Some(obj) = self.params.as_object() {
            if !obj.is_empty() {
                let param_strs: Vec<String> = obj
                    .iter()
                    .filter(|(k, _)| *k != "index") // Skip index since it's dynamic
                    .map(|(k, v)| format!("{}={}", k, format_value(v)))
                    .collect();
                if !param_strs.is_empty() {
                    parts.push(format!(" ({})", param_strs.join(", ")));
                }
            }
        }

        parts.concat()
    }
}

/// Recorded workflow in tool-based format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedWorkflow {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub start_url: String,
    pub actions: Vec<RecordedAction>,
    #[serde(default)]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl RecordedWorkflow {
    pub fn new(name: String, start_url: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description: None,
            start_url,
            actions: Vec::new(),
            created_at: chrono::Utc::now(),
        }
    }

    /// Convert recorded actions to hint text for AI prompt
    pub fn to_hints_prompt(&self) -> String {
        let mut lines = vec![
            format!("Recorded workflow: {}", self.name),
            format!("Start URL: {}", self.start_url),
            "Steps:".to_string(),
        ];

        for action in &self.actions {
            lines.push(action.to_hint_string());
        }

        lines.join("\n")
    }

    /// Convert to JSON for metadata storage
    pub fn to_hints_json(&self) -> Value {
        json!({
            "workflow_name": self.name,
            "start_url": self.start_url,
            "recorded_actions": self.actions,
            "hint_text": self.to_hints_prompt()
        })
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

fn format_value(v: &Value) -> String {
    match v {
        Value::String(s) => format!("'{}'", truncate(s, 30)),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => v.to_string(),
    }
}

// Conversion from old WorkflowStep format
use super::workflow::{ActionType, WorkflowStep};

impl From<&WorkflowStep> for RecordedAction {
    fn from(step: &WorkflowStep) -> Self {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let selector = step.action.selector.as_ref().map(|s| s.value.as_str());

        match step.action.action_type {
            ActionType::Navigate => {
                RecordedAction::navigate(
                    step.order,
                    step.action.url.as_deref().unwrap_or(""),
                    timestamp,
                )
            }
            ActionType::Click => {
                let coords = step.action.coordinates.as_ref().map(|c| (c.x, c.y));
                RecordedAction::click(
                    step.order,
                    selector,
                    None, // text not stored in old format
                    coords,
                    timestamp,
                )
            }
            ActionType::Type => {
                RecordedAction::input(
                    step.order,
                    selector,
                    step.action.value.as_deref().unwrap_or(""),
                    None,
                    timestamp,
                )
            }
            ActionType::Select => {
                RecordedAction::select(
                    step.order,
                    selector,
                    step.action.value.as_deref().unwrap_or(""),
                    timestamp,
                )
            }
            ActionType::Scroll => {
                RecordedAction::scroll(step.order, "down", 500, timestamp)
            }
            ActionType::Wait => {
                RecordedAction {
                    order: step.order,
                    tool: "wait".to_string(),
                    params: json!({ "seconds": step.wait_after_ms / 1000 }),
                    hints: None,
                    screenshot: None,
                    timestamp,
                }
            }
            ActionType::Screenshot => {
                RecordedAction {
                    order: step.order,
                    tool: "screenshot".to_string(),
                    params: json!({}),
                    hints: None,
                    screenshot: step.screenshot_after.clone(),
                    timestamp,
                }
            }
            ActionType::Extract | ActionType::Hover | ActionType::Custom => {
                // Fallback for unsupported action types
                RecordedAction {
                    order: step.order,
                    tool: "wait".to_string(),
                    params: json!({ "seconds": 1 }),
                    hints: Some(ActionHints {
                        description: Some(format!("Unsupported action: {:?}", step.action.action_type)),
                        ..Default::default()
                    }),
                    screenshot: None,
                    timestamp,
                }
            }
        }
    }
}
