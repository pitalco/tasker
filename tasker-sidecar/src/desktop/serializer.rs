use super::types::{OSElement, OSElementMap, WindowInfo};

/// Format OS elements for LLM consumption
///
/// Creates a human-readable representation similar to browser DOM extraction.
/// Example output:
/// ```text
/// [1] Button "OK" - Click to confirm
/// [2] TextField "Username" value="john" - Enter your username
/// [3] CheckBox "Remember me" checked - Remember login
/// ```
pub fn format_for_llm(element_map: &OSElementMap, window: &WindowInfo) -> String {
    let mut lines = Vec::new();

    // Add window context header
    lines.push(format!(
        "Window: {} ({})",
        window.title,
        window.process_name
    ));
    lines.push(format!(
        "Size: {}x{} at ({}, {})",
        window.bounds.width as i32,
        window.bounds.height as i32,
        window.bounds.x as i32,
        window.bounds.y as i32
    ));
    lines.push(String::new()); // Empty line
    lines.push("Interactive Elements:".to_string());

    // Format each element
    for element in &element_map.ordered_elements {
        lines.push(format_element(element));
    }

    if element_map.is_empty() {
        lines.push("  (no interactive elements found)".to_string());
    }

    lines.join("\n")
}

/// Format a single OS element for display
pub fn format_element(element: &OSElement) -> String {
    let mut parts = Vec::new();

    // Index
    parts.push(format!("[{}]", element.index));

    // Control type
    parts.push(element.control_type.clone());

    // Name (quoted)
    if let Some(ref name) = element.name {
        if !name.is_empty() {
            parts.push(format!("\"{}\"", truncate_str(name, 40)));
        }
    }

    // Value for text fields
    if let Some(ref value) = element.value {
        if !value.is_empty() && element.is_editable {
            parts.push(format!("value=\"{}\"", truncate_str(value, 30)));
        }
    }

    // State indicators
    let mut states = Vec::new();

    if !element.is_enabled {
        states.push("disabled");
    }

    if element.is_focused {
        states.push("focused");
    }

    if let Some(checked) = element.is_checked {
        if checked {
            states.push("checked");
        }
    }

    if let Some(selected) = element.is_selected {
        if selected {
            states.push("selected");
        }
    }

    if let Some(expanded) = element.is_expanded {
        states.push(if expanded { "expanded" } else { "collapsed" });
    }

    if !states.is_empty() {
        parts.push(format!("[{}]", states.join(", ")));
    }

    // Keyboard shortcut
    if let Some(ref key) = element.accelerator_key {
        if !key.is_empty() {
            parts.push(format!("({})", key));
        }
    }

    parts.join(" ")
}

/// Format elements as a compact list for tool results
pub fn format_elements_compact(elements: &[OSElement]) -> String {
    elements
        .iter()
        .map(|e| {
            let name = e.name.as_deref().unwrap_or(&e.control_type);
            format!("[{}] {} \"{}\"", e.index, e.control_type, truncate_str(name, 30))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format window list for tool results
pub fn format_windows(windows: &[WindowInfo]) -> String {
    let mut lines = Vec::new();

    for (i, window) in windows.iter().enumerate() {
        let mut parts = Vec::new();

        parts.push(format!("[{}]", i + 1));
        parts.push(format!("\"{}\"", truncate_str(&window.title, 50)));
        parts.push(format!("({})", window.process_name));

        if window.is_focused {
            parts.push("[focused]".to_string());
        }

        if window.is_minimized {
            parts.push("[minimized]".to_string());
        }

        lines.push(parts.join(" "));
    }

    if lines.is_empty() {
        "No visible windows found".to_string()
    } else {
        lines.join("\n")
    }
}

/// Format element details for debugging
pub fn format_element_details(element: &OSElement) -> String {
    let mut lines = Vec::new();

    lines.push(format!("ID: {}", element.id));
    lines.push(format!("Index: [{}]", element.index));
    lines.push(format!("Type: {}", element.control_type));

    if let Some(ref name) = element.name {
        lines.push(format!("Name: {}", name));
    }

    if let Some(ref value) = element.value {
        lines.push(format!("Value: {}", value));
    }

    if let Some(ref desc) = element.description {
        lines.push(format!("Description: {}", desc));
    }

    lines.push(format!(
        "Bounds: ({}, {}) {}x{}",
        element.bounds.x as i32,
        element.bounds.y as i32,
        element.bounds.width as i32,
        element.bounds.height as i32
    ));

    lines.push(format!("Enabled: {}", element.is_enabled));
    lines.push(format!("Focusable: {}", element.is_focusable));
    lines.push(format!("Editable: {}", element.is_editable));

    if !element.patterns.is_empty() {
        lines.push(format!("Patterns: {}", element.patterns.join(", ")));
    }

    if let Some(ref aid) = element.automation_id {
        lines.push(format!("AutomationID: {}", aid));
    }

    lines.join("\n")
}

/// Truncate a string to a maximum length, adding "..." if truncated
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::types::OSRect;

    #[test]
    fn test_format_element() {
        let element = OSElement {
            id: "test".to_string(),
            index: 1,
            control_type: "Button".to_string(),
            name: Some("OK".to_string()),
            value: None,
            description: None,
            bounds: OSRect::new(100.0, 200.0, 80.0, 30.0),
            is_enabled: true,
            is_focusable: true,
            is_focused: false,
            is_editable: false,
            is_checked: None,
            is_selected: None,
            is_expanded: None,
            window_id: "win1".to_string(),
            automation_id: None,
            role: None,
            patterns: vec!["Invoke".to_string()],
            accelerator_key: None,
            access_key: None,
        };

        let formatted = format_element(&element);
        assert!(formatted.contains("[1]"));
        assert!(formatted.contains("Button"));
        assert!(formatted.contains("\"OK\""));
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 8), "hello...");
    }
}
