use super::models::*;
use crate::db::{WorkflowDto, WorkflowMetadata, WorkflowStep, WorkflowVariable};

/// Convert a Taskfile to a WorkflowDto for storage
pub fn taskfile_to_workflow(taskfile: &Taskfile) -> WorkflowDto {
    let steps: Vec<WorkflowStep> = taskfile
        .steps
        .iter()
        .enumerate()
        .map(|(i, step)| {
            let action = taskfile_action_to_json(&step.action);
            let name = step.description.clone().unwrap_or_else(|| {
                action_to_name(&step.action)
            });
            WorkflowStep {
                id: step.id.clone(),
                order: i as i32,
                name,
                action,
                description: step.description.clone(),
                screenshot_path: None,
                dom_snapshot: None,
            }
        })
        .collect();

    let variables: Vec<WorkflowVariable> = taskfile
        .variables
        .iter()
        .map(|v| WorkflowVariable {
            name: v.name.clone(),
            var_type: v.var_type.clone(),
            default_value: v.default.clone(),
        })
        .collect();

    // Determine start_url from first navigate step
    let start_url = taskfile.steps.iter().find_map(|step| {
        if let TaskfileAction::Navigate { url } = &step.action {
            Some(url.clone())
        } else {
            None
        }
    });

    let llm_provider = taskfile
        .execution
        .llm
        .as_ref()
        .map(|llm| llm.provider.clone());

    WorkflowDto {
        id: String::new(), // Will be set by repository
        name: taskfile.metadata.name.clone(),
        steps,
        variables,
        metadata: WorkflowMetadata {
            start_url,
            llm_provider,
            recording_source: "imported".to_string(),
        },
        created_at: String::new(), // Will be set by repository
        updated_at: String::new(), // Will be set by repository
        version: 1,
        task_description: taskfile.metadata.description.clone(),
        stop_when: None,
        max_steps: None,
    }
}

/// Convert a WorkflowDto to a Taskfile for export
pub fn workflow_to_taskfile(workflow: &WorkflowDto) -> Taskfile {
    let steps: Vec<TaskfileStep> = workflow
        .steps
        .iter()
        .map(|step| TaskfileStep {
            id: step.id.clone(),
            action: json_to_taskfile_action(&step.action),
            description: step.description.clone(),
            condition: None,
        })
        .collect();

    let variables: Vec<Variable> = workflow
        .variables
        .iter()
        .map(|v| Variable {
            name: v.name.clone(),
            var_type: v.var_type.clone(),
            required: false,
            default: v.default_value.clone(),
            description: None,
        })
        .collect();

    let llm_config = workflow.metadata.llm_provider.as_ref().map(|provider| {
        LLMExecutionConfig {
            provider: provider.clone(),
            model: "gemini-2.5-flash".to_string(), // Default model
            api_key_env: Some(format!("{}_API_KEY", provider.to_uppercase())),
        }
    });

    Taskfile {
        taskfile: "1.0".to_string(),
        metadata: TaskfileMetadata {
            name: workflow.name.clone(),
            description: workflow.task_description.clone(),
            version: format!("1.0.{}", workflow.version),
            author: None,
            tags: vec![],
        },
        triggers: Triggers::default(),
        dependencies: Dependencies {
            browser: BrowserDependency::default(),
            env: vec![],
            accounts: vec![],
        },
        limits: Limits::default(),
        variables,
        execution: ExecutionConfig {
            mode: "ai_assisted".to_string(),
            llm: llm_config,
            retry: RetryConfig::default(),
        },
        steps,
        output: Output::default(),
    }
}

/// Generate a human-readable name from a TaskfileAction
fn action_to_name(action: &TaskfileAction) -> String {
    match action {
        TaskfileAction::Navigate { url } => {
            format!("Navigate to {}", truncate_string(url, 40))
        }
        TaskfileAction::Click { selector } => {
            let target = selector_to_description(selector);
            format!("Click {}", target)
        }
        TaskfileAction::Type { text, .. } => {
            format!("Type '{}'", truncate_string(text, 30))
        }
        TaskfileAction::Wait { condition } => {
            match condition {
                WaitCondition::Delay { ms } => format!("Wait {}ms", ms),
                WaitCondition::UrlMatch { value, .. } => format!("Wait for URL: {}", truncate_string(value, 30)),
                WaitCondition::ElementVisible { .. } => "Wait for element visible".to_string(),
                WaitCondition::ElementHidden { .. } => "Wait for element hidden".to_string(),
            }
        }
        TaskfileAction::Extract { variable, .. } => {
            format!("Extract to '{}'", variable)
        }
        TaskfileAction::Screenshot { .. } => "Take screenshot".to_string(),
        TaskfileAction::Scroll { direction, .. } => {
            format!("Scroll {}", direction)
        }
        TaskfileAction::Select { value, .. } => {
            format!("Select '{}'", truncate_string(value, 30))
        }
        TaskfileAction::Hover { selector } => {
            let target = selector_to_description(selector);
            format!("Hover over {}", target)
        }
        TaskfileAction::Custom { prompt } => {
            format!("Custom: {}", truncate_string(prompt, 40))
        }
    }
}

fn selector_to_description(selector: &Selector) -> String {
    if let Some(text) = &selector.text {
        return format!("'{}'", truncate_string(text, 25));
    }
    if let Some(aria) = &selector.aria_label {
        return format!("'{}'", truncate_string(aria, 25));
    }
    if let Some(css) = &selector.css {
        return truncate_string(css, 30);
    }
    "element".to_string()
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

/// Convert TaskfileAction to JSON Value for storage
fn taskfile_action_to_json(action: &TaskfileAction) -> serde_json::Value {
    match action {
        TaskfileAction::Navigate { url } => {
            serde_json::json!({
                "type": "navigate",
                "url": url
            })
        }
        TaskfileAction::Click { selector } => {
            serde_json::json!({
                "type": "click",
                "selector": selector_to_json(selector)
            })
        }
        TaskfileAction::Type {
            selector,
            text,
            clear_first,
        } => {
            serde_json::json!({
                "type": "type",
                "selector": selector_to_json(selector),
                "text": text,
                "clear_first": clear_first
            })
        }
        TaskfileAction::Wait { condition } => {
            serde_json::json!({
                "type": "wait",
                "condition": wait_condition_to_json(condition)
            })
        }
        TaskfileAction::Extract {
            selector,
            attribute,
            variable,
        } => {
            serde_json::json!({
                "type": "extract",
                "selector": selector_to_json(selector),
                "attribute": attribute,
                "variable": variable
            })
        }
        TaskfileAction::Screenshot {
            full_page,
            variable,
        } => {
            serde_json::json!({
                "type": "screenshot",
                "full_page": full_page,
                "variable": variable
            })
        }
        TaskfileAction::Scroll { direction, amount } => {
            serde_json::json!({
                "type": "scroll",
                "direction": direction,
                "amount": amount
            })
        }
        TaskfileAction::Select { selector, value } => {
            serde_json::json!({
                "type": "select",
                "selector": selector_to_json(selector),
                "value": value
            })
        }
        TaskfileAction::Hover { selector } => {
            serde_json::json!({
                "type": "hover",
                "selector": selector_to_json(selector)
            })
        }
        TaskfileAction::Custom { prompt } => {
            serde_json::json!({
                "type": "custom",
                "prompt": prompt
            })
        }
    }
}

fn selector_to_json(selector: &Selector) -> serde_json::Value {
    serde_json::json!({
        "css": selector.css,
        "xpath": selector.xpath,
        "text": selector.text,
        "aria_label": selector.aria_label
    })
}

fn wait_condition_to_json(condition: &WaitCondition) -> serde_json::Value {
    match condition {
        WaitCondition::UrlMatch { value, timeout_ms } => {
            serde_json::json!({
                "type": "url_match",
                "value": value,
                "timeout_ms": timeout_ms
            })
        }
        WaitCondition::ElementVisible {
            selector,
            timeout_ms,
        } => {
            serde_json::json!({
                "type": "element_visible",
                "selector": selector_to_json(selector),
                "timeout_ms": timeout_ms
            })
        }
        WaitCondition::ElementHidden {
            selector,
            timeout_ms,
        } => {
            serde_json::json!({
                "type": "element_hidden",
                "selector": selector_to_json(selector),
                "timeout_ms": timeout_ms
            })
        }
        WaitCondition::Delay { ms } => {
            serde_json::json!({
                "type": "delay",
                "ms": ms
            })
        }
    }
}

/// Convert JSON Value back to TaskfileAction
fn json_to_taskfile_action(json: &serde_json::Value) -> TaskfileAction {
    let action_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match action_type {
        "navigate" => TaskfileAction::Navigate {
            url: json
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
        "click" => TaskfileAction::Click {
            selector: json_to_selector(json.get("selector")),
        },
        "type" => TaskfileAction::Type {
            selector: json_to_selector(json.get("selector")),
            text: json
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            clear_first: json.get("clear_first").and_then(|v| v.as_bool()).unwrap_or(false),
        },
        "wait" => {
            let condition = json.get("condition");
            let cond_type = condition
                .and_then(|c| c.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("delay");

            let wait_cond = match cond_type {
                "url_match" => WaitCondition::UrlMatch {
                    value: condition
                        .and_then(|c| c.get("value"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    timeout_ms: condition
                        .and_then(|c| c.get("timeout_ms"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(10000) as u32,
                },
                "element_visible" => WaitCondition::ElementVisible {
                    selector: json_to_selector(condition.and_then(|c| c.get("selector"))),
                    timeout_ms: condition
                        .and_then(|c| c.get("timeout_ms"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(10000) as u32,
                },
                "element_hidden" => WaitCondition::ElementHidden {
                    selector: json_to_selector(condition.and_then(|c| c.get("selector"))),
                    timeout_ms: condition
                        .and_then(|c| c.get("timeout_ms"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(10000) as u32,
                },
                _ => WaitCondition::Delay {
                    ms: condition
                        .and_then(|c| c.get("ms"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(1000) as u32,
                },
            };
            TaskfileAction::Wait { condition: wait_cond }
        }
        "extract" => TaskfileAction::Extract {
            selector: json_to_selector(json.get("selector")),
            attribute: json
                .get("attribute")
                .and_then(|v| v.as_str())
                .unwrap_or("textContent")
                .to_string(),
            variable: json
                .get("variable")
                .and_then(|v| v.as_str())
                .unwrap_or("extracted")
                .to_string(),
        },
        "screenshot" => TaskfileAction::Screenshot {
            full_page: json.get("full_page").and_then(|v| v.as_bool()).unwrap_or(false),
            variable: json.get("variable").and_then(|v| v.as_str()).map(String::from),
        },
        "scroll" => TaskfileAction::Scroll {
            direction: json
                .get("direction")
                .and_then(|v| v.as_str())
                .unwrap_or("down")
                .to_string(),
            amount: json.get("amount").and_then(|v| v.as_i64()).map(|v| v as i32),
        },
        "select" => TaskfileAction::Select {
            selector: json_to_selector(json.get("selector")),
            value: json
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
        "hover" => TaskfileAction::Hover {
            selector: json_to_selector(json.get("selector")),
        },
        "custom" => TaskfileAction::Custom {
            prompt: json
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
        _ => TaskfileAction::Custom {
            prompt: format!("Unknown action type: {}", action_type),
        },
    }
}

fn json_to_selector(json: Option<&serde_json::Value>) -> Selector {
    match json {
        Some(v) => Selector {
            css: v.get("css").and_then(|s| s.as_str()).map(String::from),
            xpath: v.get("xpath").and_then(|s| s.as_str()).map(String::from),
            text: v.get("text").and_then(|s| s.as_str()).map(String::from),
            aria_label: v.get("aria_label").and_then(|s| s.as_str()).map(String::from),
        },
        None => Selector {
            css: None,
            xpath: None,
            text: None,
            aria_label: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_navigate_action() {
        let action = TaskfileAction::Navigate {
            url: "https://example.com".to_string(),
        };
        let json = taskfile_action_to_json(&action);
        let back = json_to_taskfile_action(&json);

        match back {
            TaskfileAction::Navigate { url } => assert_eq!(url, "https://example.com"),
            _ => panic!("Expected Navigate action"),
        }
    }
}
