use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents an interactive element on the page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveElement {
    pub index: i32,  // 1-based index for AI interaction
    pub tag: String,
    pub selector: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aria_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub visible: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Indexed element state for the current page
/// Maps index (1-based) to element selector for tool execution
#[derive(Debug, Clone, Default)]
pub struct IndexedElements {
    pub elements: Vec<InteractiveElement>,
    pub index_to_selector: HashMap<i32, String>,
}

impl IndexedElements {
    pub fn new(elements: Vec<InteractiveElement>) -> Self {
        let mut index_to_selector = HashMap::new();
        for el in &elements {
            index_to_selector.insert(el.index, el.selector.clone());
        }
        Self { elements, index_to_selector }
    }

    /// Get selector for an element by index (1-based)
    pub fn get_selector(&self, index: i32) -> Option<&str> {
        self.index_to_selector.get(&index).map(|s| s.as_str())
    }

    /// Get element by index (1-based)
    pub fn get_element(&self, index: i32) -> Option<&InteractiveElement> {
        self.elements.iter().find(|e| e.index == index)
    }

    /// Format for LLM
    pub fn format_for_llm(&self) -> String {
        format_elements_for_llm(&self.elements)
    }
}

/// JavaScript to extract interactive elements from the page
/// Returns elements with 1-based indices for AI interaction
pub const EXTRACT_ELEMENTS_SCRIPT: &str = r#"
(() => {
    const isVisible = (el) => {
        const rect = el.getBoundingClientRect();
        const style = window.getComputedStyle(el);
        return rect.width > 0 &&
               rect.height > 0 &&
               style.display !== 'none' &&
               style.visibility !== 'hidden' &&
               style.opacity !== '0';
    };

    const getSelector = (el) => {
        if (el.id) return '#' + CSS.escape(el.id);

        let selector = el.tagName.toLowerCase();

        if (el.getAttribute('data-testid')) {
            return `[data-testid="${el.getAttribute('data-testid')}"]`;
        }

        if (el.getAttribute('aria-label')) {
            return `[aria-label="${el.getAttribute('aria-label')}"]`;
        }

        if (el.name) {
            return `${selector}[name="${el.name}"]`;
        }

        if (el.className && typeof el.className === 'string') {
            const classes = el.className.trim().split(/\s+/).slice(0, 2);
            if (classes.length > 0 && classes[0]) {
                selector += '.' + classes.map(c => CSS.escape(c)).join('.');
            }
        }

        // Add nth-child if needed for uniqueness
        const parent = el.parentElement;
        if (parent) {
            const siblings = Array.from(parent.children).filter(c => c.tagName === el.tagName);
            if (siblings.length > 1) {
                const idx = siblings.indexOf(el) + 1;
                selector += `:nth-child(${idx})`;
            }
        }

        return selector;
    };

    const interactiveTags = ['a', 'button', 'input', 'select', 'textarea', 'label'];
    const interactiveRoles = ['button', 'link', 'checkbox', 'radio', 'textbox', 'tab', 'menuitem'];

    const elements = [];
    const seenSelectors = new Set();

    const addElement = (el) => {
        const selector = getSelector(el);
        if (seenSelectors.has(selector)) return;
        seenSelectors.add(selector);

        const rect = el.getBoundingClientRect();
        elements.push({
            index: elements.length + 1,  // 1-based index
            tag: el.tagName.toLowerCase(),
            selector: selector,
            id: el.id || null,
            class: el.className || null,
            text: el.textContent?.trim().substring(0, 100) || null,
            href: el.href || null,
            aria_label: el.getAttribute('aria-label') || null,
            placeholder: el.placeholder || null,
            value: el.value || null,
            input_type: el.type || null,
            name: el.name || null,
            visible: true,
            x: Math.round(rect.x),
            y: Math.round(rect.y),
            width: Math.round(rect.width),
            height: Math.round(rect.height)
        });
    };

    // Get elements by tag
    interactiveTags.forEach(tag => {
        document.querySelectorAll(tag).forEach(el => {
            if (isVisible(el)) addElement(el);
        });
    });

    // Get elements by role
    interactiveRoles.forEach(role => {
        document.querySelectorAll(`[role="${role}"]`).forEach(el => {
            if (isVisible(el)) addElement(el);
        });
    });

    // Get clickable elements with onclick
    document.querySelectorAll('[onclick]').forEach(el => {
        if (isVisible(el)) addElement(el);
    });

    return elements;
})()
"#;

/// Simplified page state for LLM context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageState {
    pub url: String,
    pub title: String,
    pub elements: Vec<InteractiveElement>,
}

/// JavaScript to get basic page info
pub const GET_PAGE_INFO_SCRIPT: &str = r#"
(() => {
    return {
        url: window.location.href,
        title: document.title
    };
})()
"#;

/// Parse interactive elements from JSON value
pub fn parse_elements(value: &serde_json::Value) -> Result<Vec<InteractiveElement>> {
    let elements: Vec<InteractiveElement> = serde_json::from_value(value.clone())
        .map_err(|e| anyhow!("Failed to parse elements: {}", e))?;
    Ok(elements)
}

/// Format elements for LLM context
/// Output format: [1]<input type="text" placeholder="Search">
pub fn format_elements_for_llm(elements: &[InteractiveElement]) -> String {
    let mut output = String::new();

    for el in elements {
        // Build attributes string
        let mut attrs = Vec::new();

        // Input type first (most important for inputs)
        if let Some(input_type) = &el.input_type {
            if !input_type.is_empty() && input_type != "text" {
                attrs.push(format!("type=\"{}\"", input_type));
            }
        }

        // Name attribute
        if let Some(name) = &el.name {
            if !name.is_empty() {
                attrs.push(format!("name=\"{}\"", name));
            }
        }

        // Placeholder
        if let Some(placeholder) = &el.placeholder {
            if !placeholder.is_empty() {
                attrs.push(format!("placeholder=\"{}\"", placeholder));
            }
        }

        // Aria-label
        if let Some(aria_label) = &el.aria_label {
            if !aria_label.is_empty() {
                attrs.push(format!("aria-label=\"{}\"", aria_label));
            }
        }

        // Href for links
        if let Some(href) = &el.href {
            if !href.is_empty() {
                // Truncate long URLs
                let display_href = if href.len() > 50 {
                    format!("{}...", &href[..50])
                } else {
                    href.clone()
                };
                attrs.push(format!("href=\"{}\"", display_href));
            }
        }

        // Build the opening tag
        let attr_str = if attrs.is_empty() {
            String::new()
        } else {
            format!(" {}", attrs.join(" "))
        };

        // Text content (truncated)
        let text_content = el.text.as_ref()
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .map(|t| {
                if t.len() > 50 {
                    format!("{}...", &t[..50])
                } else {
                    t.to_string()
                }
            });

        // Format: [index]<tag attrs>text</tag> or [index]<tag attrs />
        let line = if let Some(text) = text_content {
            format!("[{}]<{}{}>{}</{}>", el.index, el.tag, attr_str, text, el.tag)
        } else {
            format!("[{}]<{}{} />", el.index, el.tag, attr_str)
        };

        output.push_str(&line);
        output.push('\n');
    }

    output
}
