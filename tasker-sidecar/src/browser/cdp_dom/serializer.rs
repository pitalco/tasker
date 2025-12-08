use super::types::{ElementIndex, EnhancedDOMNode, SelectOption, SelectorMap, SimplifiedElement};

/// Extract interactive elements from tree and build selector map
pub fn extract_interactive_elements(tree: &EnhancedDOMNode) -> SelectorMap {
    let mut elements = Vec::new();
    collect_interactive(tree, &mut elements);

    // Sort by position (top to bottom, left to right)
    elements.sort_by(|a, b| {
        let ay = a.bounds.y as i64;
        let by = b.bounds.y as i64;
        // Group elements within 20px vertical distance
        if (ay - by).abs() > 20 {
            ay.cmp(&by)
        } else {
            (a.bounds.x as i64).cmp(&(b.bounds.x as i64))
        }
    });

    // Assign 1-based indices and build map
    let mut selector_map = SelectorMap::new();
    for (i, mut elem) in elements.into_iter().enumerate() {
        let index = (i + 1) as ElementIndex;
        elem.index = index;
        selector_map.index_to_backend_id.insert(index, elem.backend_node_id);
        selector_map.backend_id_to_element.insert(elem.backend_node_id, elem.clone());
        selector_map.ordered_elements.push(elem);
    }

    selector_map
}

/// Recursively collect interactive elements
fn collect_interactive(node: &EnhancedDOMNode, elements: &mut Vec<SimplifiedElement>) {
    // Only collect visible, interactive, non-obscured elements
    if node.is_interactive && node.is_visible && !node.is_obscured {
        let elem = node_to_simplified(node);
        elements.push(elem);
    }

    for child in &node.children {
        collect_interactive(child, elements);
    }
}

/// Convert EnhancedDOMNode to SimplifiedElement
#[allow(clippy::field_reassign_with_default)]
fn node_to_simplified(node: &EnhancedDOMNode) -> SimplifiedElement {
    let mut elem = SimplifiedElement::default();

    elem.backend_node_id = node.backend_node_id;
    elem.tag = node.tag_name.clone();
    elem.in_shadow_dom = node.in_shadow_dom;
    elem.frame_id = node.frame_id.clone();

    // Get bounds from layout
    if let Some(layout) = &node.layout {
        elem.bounds = layout.bounds.clone();
    }

    // Extract from attributes
    elem.input_type = node.attributes.get("type").cloned();
    elem.value = node.attributes.get("value").cloned();
    elem.placeholder = node.attributes.get("placeholder").cloned();
    elem.aria_label = node.attributes.get("aria-label").cloned();
    elem.title = node.attributes.get("title").cloned();
    elem.href = node.attributes.get("href").cloned();

    // Boolean attributes
    elem.disabled = node.attributes.contains_key("disabled").then_some(true);
    elem.required = node.attributes.contains_key("required").then_some(true);
    elem.readonly = node.attributes.contains_key("readonly").then_some(true);
    elem.checked = node.attributes.get("checked").map(|_| true);
    elem.selected = node.attributes.get("selected").map(|_| true);

    // Get role from attributes
    elem.role = node.attributes.get("role").cloned();

    // Override with AX data if available (more accurate)
    if let Some(ax) = &node.ax_data {
        if ax.role.is_some() {
            elem.role = ax.role.clone();
        }
        if ax.name.is_some() {
            elem.name = ax.name.clone();
        }
        if let Some(val) = &ax.value {
            if !val.is_empty() {
                elem.value = Some(val.clone());
            }
        }
        if ax.checked.is_some() {
            elem.checked = ax.checked;
        }
        if ax.selected.is_some() {
            elem.selected = ax.selected;
        }
        if ax.disabled.is_some() {
            elem.disabled = ax.disabled;
        }
        if ax.required {
            elem.required = Some(true);
        }
        if ax.readonly {
            elem.readonly = Some(true);
        }
    }

    // Get text content from children or node itself
    elem.text = get_text_content(node);

    // For select elements, extract options from children
    if node.tag_name.eq_ignore_ascii_case("select") {
        elem.select_options = Some(extract_select_options(node));
    }

    elem
}

/// Extract options from a select element's children
fn extract_select_options(node: &EnhancedDOMNode) -> Vec<SelectOption> {
    let mut options = Vec::new();

    for child in &node.children {
        if child.tag_name.eq_ignore_ascii_case("option") {
            let value = child.attributes.get("value").cloned().unwrap_or_default();
            let selected = child.attributes.contains_key("selected");

            // Get text from child text nodes
            let text = child
                .children
                .iter()
                .filter(|c| c.node_type == 3) // TEXT_NODE
                .filter_map(|c| c.text_content.as_deref())
                .collect::<Vec<_>>()
                .join("")
                .trim()
                .to_string();

            // Use value as text if text is empty
            let text = if text.is_empty() {
                value.clone()
            } else {
                text
            };

            options.push(SelectOption {
                value,
                text,
                selected,
            });
        } else if child.tag_name.eq_ignore_ascii_case("optgroup") {
            // Handle optgroup - extract options from within
            for opt_child in &child.children {
                if opt_child.tag_name.eq_ignore_ascii_case("option") {
                    let value = opt_child.attributes.get("value").cloned().unwrap_or_default();
                    let selected = opt_child.attributes.contains_key("selected");

                    let text = opt_child
                        .children
                        .iter()
                        .filter(|c| c.node_type == 3)
                        .filter_map(|c| c.text_content.as_deref())
                        .collect::<Vec<_>>()
                        .join("")
                        .trim()
                        .to_string();

                    let text = if text.is_empty() {
                        value.clone()
                    } else {
                        text
                    };

                    options.push(SelectOption {
                        value,
                        text,
                        selected,
                    });
                }
            }
        }
    }

    options
}

/// Get text content from a node (direct children only)
fn get_text_content(node: &EnhancedDOMNode) -> Option<String> {
    // For input elements, don't extract text
    let input_tags = ["input", "textarea", "select"];
    if input_tags.contains(&node.tag_name.as_str()) {
        return None;
    }

    let mut text = String::new();

    // Collect text from text node children
    for child in &node.children {
        if child.node_type == 3 {
            // TEXT_NODE
            if let Some(t) = &child.text_content {
                text.push_str(t);
                text.push(' ');
            }
        } else if child.node_type == 1 {
            // ELEMENT_NODE - get text from immediate children only
            for grandchild in &child.children {
                if grandchild.node_type == 3 {
                    if let Some(t) = &grandchild.text_content {
                        text.push_str(t);
                        text.push(' ');
                    }
                }
            }
        }
    }

    let text = text.trim().replace(char::is_whitespace, " ");
    let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");

    if text.is_empty() {
        None
    } else {
        // Truncate to 100 chars
        Some(truncate_str(&text, 100))
    }
}

/// Safely truncate string at character boundary
fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect::<String>() + "..."
    }
}

/// Format selector map for LLM context
pub fn format_for_llm(selector_map: &SelectorMap) -> String {
    let mut output = String::new();

    for elem in &selector_map.ordered_elements {
        let mut attrs: Vec<String> = Vec::new();

        // Type (skip default "text" for inputs)
        if let Some(t) = &elem.input_type {
            if !t.is_empty() && t != "text" && t != "submit" {
                attrs.push(format!("type={}", t));
            }
        }

        // Name
        if let Some(n) = &elem.name {
            if !n.is_empty() {
                attrs.push(format!("name={}", truncate_str(n, 30)));
            }
        }

        // Role (skip if matches tag)
        if let Some(r) = &elem.role {
            if !r.is_empty() && r.to_lowercase() != elem.tag {
                attrs.push(format!("role={}", r));
            }
        }

        // Value (for form elements)
        if let Some(v) = &elem.value {
            if !v.is_empty() && elem.tag != "button" {
                attrs.push(format!("value={}", truncate_str(v, 30)));
            }
        }

        // Options for select elements - show all available choices
        if let Some(options) = &elem.select_options {
            if !options.is_empty() {
                let opt_texts: Vec<&str> = options.iter().map(|o| o.text.as_str()).collect();
                attrs.push(format!("options=\"{}\"", opt_texts.join(",")));
            }
        }

        // Boolean states
        if elem.checked == Some(true) {
            attrs.push("checked".to_string());
        }
        if elem.selected == Some(true) {
            attrs.push("selected".to_string());
        }
        if elem.disabled == Some(true) {
            attrs.push("disabled".to_string());
        }
        if elem.required == Some(true) {
            attrs.push("required".to_string());
        }
        if elem.readonly == Some(true) {
            attrs.push("readonly".to_string());
        }

        // Placeholder
        if let Some(p) = &elem.placeholder {
            if !p.is_empty() {
                attrs.push(format!("placeholder={}", truncate_str(p, 30)));
            }
        }

        // Aria-label (skip if same as text)
        if let Some(a) = &elem.aria_label {
            if !a.is_empty() {
                let skip = elem
                    .text
                    .as_ref()
                    .map(|t| t.to_lowercase() == a.to_lowercase())
                    .unwrap_or(false);
                if !skip {
                    attrs.push(format!("aria-label={}", truncate_str(a, 40)));
                }
            }
        }

        // Title (skip if same as aria-label or text)
        if let Some(t) = &elem.title {
            if !t.is_empty() {
                let skip_aria = elem.aria_label.as_ref().map(|a| a == t).unwrap_or(false);
                let skip_text = elem
                    .text
                    .as_ref()
                    .map(|x| x.to_lowercase() == t.to_lowercase())
                    .unwrap_or(false);
                if !skip_aria && !skip_text {
                    attrs.push(format!("title={}", truncate_str(t, 40)));
                }
            }
        }

        // Href (truncated, skip javascript:)
        if let Some(h) = &elem.href {
            if !h.is_empty() && !h.starts_with("javascript:") {
                attrs.push(format!("href={}", truncate_str(h, 50)));
            }
        }

        // Build line
        let attr_str = if attrs.is_empty() {
            String::new()
        } else {
            format!(" {}", attrs.join(" "))
        };

        // Add position info: @(x,y) for spatial awareness
        let pos = format!("@({},{})", elem.bounds.x as i32, elem.bounds.y as i32);

        let text = elem
            .text
            .as_ref()
            .map(|t| truncate_str(t.trim(), 50))
            .filter(|t| !t.is_empty());

        let line = match text {
            Some(t) => format!("[{}]<{}{} {}>{}", elem.index, elem.tag, attr_str, pos, t),
            None => format!("[{}]<{}{} {} />", elem.index, elem.tag, attr_str, pos),
        };

        output.push_str(&line);
        output.push('\n');
    }

    output
}
