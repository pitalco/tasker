use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents an interactive element on the page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveElement {
    pub index: i32,
    pub tag: String,
    pub selector: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aria_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readonly: Option<bool>,
    pub visible: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Indexed element state for the current page
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

    pub fn get_selector(&self, index: i32) -> Option<&str> {
        self.index_to_selector.get(&index).map(|s| s.as_str())
    }

    pub fn get_element(&self, index: i32) -> Option<&InteractiveElement> {
        self.elements.iter().find(|e| e.index == index)
    }

    pub fn format_for_llm(&self) -> String {
        format_elements_for_llm(&self.elements)
    }
}

/// JavaScript to extract interactive elements from the page
pub const EXTRACT_ELEMENTS_SCRIPT: &str = r#"
(() => {
    const isVisible = (el) => {
        const rect = el.getBoundingClientRect();
        if (rect.width === 0 || rect.height === 0) return false;

        const style = window.getComputedStyle(el);
        if (style.display === 'none') return false;
        if (style.visibility === 'hidden') return false;
        if (style.opacity === '0') return false;

        // Check if element is in viewport
        const inViewport = rect.top < window.innerHeight &&
                          rect.bottom > 0 &&
                          rect.left < window.innerWidth &&
                          rect.right > 0;
        return inViewport;
    };

    const getSelector = (el) => {
        // Prefer ID
        if (el.id) return '#' + CSS.escape(el.id);

        // Test IDs
        const testId = el.getAttribute('data-testid') ||
                       el.getAttribute('data-test') ||
                       el.getAttribute('data-cy');
        if (testId) return `[data-testid="${testId}"]`;

        // Aria label
        if (el.getAttribute('aria-label')) {
            return `[aria-label="${el.getAttribute('aria-label')}"]`;
        }

        // Name attribute
        if (el.name) {
            return `${el.tagName.toLowerCase()}[name="${el.name}"]`;
        }

        // Build selector with tag and classes
        let selector = el.tagName.toLowerCase();
        if (el.className && typeof el.className === 'string') {
            const classes = el.className.trim().split(/\s+/).filter(c => c && !c.includes(':'));
            if (classes.length > 0) {
                selector += '.' + classes.slice(0, 2).map(c => CSS.escape(c)).join('.');
            }
        }

        // Add nth-of-type for uniqueness
        const parent = el.parentElement;
        if (parent) {
            const siblings = Array.from(parent.children).filter(c => c.tagName === el.tagName);
            if (siblings.length > 1) {
                const idx = siblings.indexOf(el) + 1;
                selector += `:nth-of-type(${idx})`;
            }
        }

        return selector;
    };

    const getTextContent = (el) => {
        // For inputs, don't get text content
        if (['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName)) return null;

        // Get direct text, excluding deeply nested content
        let text = '';
        for (const node of el.childNodes) {
            if (node.nodeType === Node.TEXT_NODE) {
                text += node.textContent;
            } else if (node.nodeType === Node.ELEMENT_NODE &&
                       !['SCRIPT', 'STYLE', 'SVG'].includes(node.tagName)) {
                // Only get text from immediate children, not deep descendants
                for (const childNode of node.childNodes) {
                    if (childNode.nodeType === Node.TEXT_NODE) {
                        text += childNode.textContent;
                    }
                }
            }
        }
        text = text.trim().replace(/\s+/g, ' ');
        return text.length > 0 ? text.substring(0, 100) : null;
    };

    const interactiveTags = ['a', 'button', 'input', 'select', 'textarea', 'label', 'details', 'summary'];
    const interactiveRoles = ['button', 'link', 'checkbox', 'radio', 'textbox', 'searchbox',
                              'tab', 'tabpanel', 'menuitem', 'option', 'switch', 'slider',
                              'combobox', 'listbox', 'menu', 'menubar', 'tree', 'treeitem'];

    const elements = [];
    const seenSelectors = new Set();

    const addElement = (el) => {
        if (!isVisible(el)) return;

        const selector = getSelector(el);
        if (seenSelectors.has(selector)) return;
        seenSelectors.add(selector);

        const rect = el.getBoundingClientRect();
        const tag = el.tagName.toLowerCase();

        elements.push({
            index: elements.length + 1,
            tag: tag,
            selector: selector,
            id: el.id || null,
            role: el.getAttribute('role') || null,
            name: el.name || null,
            input_type: el.type || null,
            value: el.value || null,
            placeholder: el.placeholder || null,
            aria_label: el.getAttribute('aria-label') || null,
            title: el.title || null,
            text: getTextContent(el),
            href: el.href || null,
            checked: el.checked === true ? true : null,
            selected: el.selected === true ? true : null,
            disabled: el.disabled === true ? true : null,
            required: el.required === true ? true : null,
            readonly: el.readOnly === true ? true : null,
            visible: true,
            x: Math.round(rect.x + window.scrollX),
            y: Math.round(rect.y + window.scrollY),
            width: Math.round(rect.width),
            height: Math.round(rect.height)
        });
    };

    // Interactive tags
    interactiveTags.forEach(tag => {
        document.querySelectorAll(tag).forEach(addElement);
    });

    // Interactive roles
    interactiveRoles.forEach(role => {
        document.querySelectorAll(`[role="${role}"]`).forEach(addElement);
    });

    // Clickable elements
    document.querySelectorAll('[onclick], [tabindex="0"], [contenteditable="true"]').forEach(addElement);

    // Sort by position (top to bottom, left to right)
    elements.sort((a, b) => {
        if (Math.abs(a.y - b.y) > 20) return a.y - b.y;
        return a.x - b.x;
    });

    // Re-index after sorting
    elements.forEach((el, i) => el.index = i + 1);

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

/// Safely truncate a string at character boundaries
fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect::<String>() + "..."
    }
}

/// Format elements for LLM context
/// Output format: [index]<tag attr=value attr2=value2 />
pub fn format_elements_for_llm(elements: &[InteractiveElement]) -> String {
    let mut output = String::new();

    for el in elements {
        let mut attrs: Vec<String> = Vec::new();

        // Type (skip "text" as it's the default for inputs)
        if let Some(t) = &el.input_type {
            if !t.is_empty() && t != "text" && t != "submit" {
                attrs.push(format!("type={}", t));
            }
        }

        // Name
        if let Some(n) = &el.name {
            if !n.is_empty() {
                attrs.push(format!("name={}", n));
            }
        }

        // Role (skip if it matches the tag)
        if let Some(r) = &el.role {
            if !r.is_empty() && r != &el.tag {
                attrs.push(format!("role={}", r));
            }
        }

        // Value (for form elements)
        if let Some(v) = &el.value {
            if !v.is_empty() && el.tag != "button" {
                attrs.push(format!("value={}", truncate_str(v, 30)));
            }
        }

        // Boolean states
        if el.checked == Some(true) {
            attrs.push("checked".to_string());
        }
        if el.selected == Some(true) {
            attrs.push("selected".to_string());
        }
        if el.disabled == Some(true) {
            attrs.push("disabled".to_string());
        }
        if el.required == Some(true) {
            attrs.push("required".to_string());
        }
        if el.readonly == Some(true) {
            attrs.push("readonly".to_string());
        }

        // Placeholder
        if let Some(p) = &el.placeholder {
            if !p.is_empty() {
                attrs.push(format!("placeholder={}", truncate_str(p, 30)));
            }
        }

        // Aria-label (skip if same as text)
        if let Some(a) = &el.aria_label {
            if !a.is_empty() {
                let skip = el.text.as_ref().map(|t| t.to_lowercase() == a.to_lowercase()).unwrap_or(false);
                if !skip {
                    attrs.push(format!("aria-label={}", truncate_str(a, 40)));
                }
            }
        }

        // Title (skip if same as aria-label or text)
        if let Some(t) = &el.title {
            if !t.is_empty() {
                let skip_aria = el.aria_label.as_ref().map(|a| a == t).unwrap_or(false);
                let skip_text = el.text.as_ref().map(|x| x.to_lowercase() == t.to_lowercase()).unwrap_or(false);
                if !skip_aria && !skip_text {
                    attrs.push(format!("title={}", truncate_str(t, 40)));
                }
            }
        }

        // Href (truncated)
        if let Some(h) = &el.href {
            if !h.is_empty() && !h.starts_with("javascript:") {
                attrs.push(format!("href={}", truncate_str(h, 50)));
            }
        }

        // Build attribute string
        let attr_str = if attrs.is_empty() {
            String::new()
        } else {
            format!(" {}", attrs.join(" "))
        };

        // Text content
        let text = el.text.as_ref()
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .map(|t| truncate_str(t, 50));

        // Format: [index]<tag attrs>text</tag> or [index]<tag attrs />
        let line = match text {
            Some(t) if !t.is_empty() => format!("[{}]<{}{}>{}", el.index, el.tag, attr_str, t),
            _ => format!("[{}]<{}{} />", el.index, el.tag, attr_str),
        };

        output.push_str(&line);
        output.push('\n');
    }

    output
}
