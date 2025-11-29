use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Represents an interactive element on the page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveElement {
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
    pub visible: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// JavaScript to extract interactive elements from the page
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
                const index = siblings.indexOf(el) + 1;
                selector += `:nth-child(${index})`;
            }
        }

        return selector;
    };

    const interactiveTags = ['a', 'button', 'input', 'select', 'textarea', 'label'];
    const interactiveRoles = ['button', 'link', 'checkbox', 'radio', 'textbox', 'tab', 'menuitem'];

    const elements = [];

    // Get elements by tag
    interactiveTags.forEach(tag => {
        document.querySelectorAll(tag).forEach(el => {
            if (isVisible(el)) {
                const rect = el.getBoundingClientRect();
                elements.push({
                    tag: el.tagName.toLowerCase(),
                    selector: getSelector(el),
                    id: el.id || null,
                    class: el.className || null,
                    text: el.textContent?.trim().substring(0, 100) || null,
                    href: el.href || null,
                    aria_label: el.getAttribute('aria-label') || null,
                    placeholder: el.placeholder || null,
                    value: el.value || null,
                    input_type: el.type || null,
                    visible: true,
                    x: Math.round(rect.x),
                    y: Math.round(rect.y),
                    width: Math.round(rect.width),
                    height: Math.round(rect.height)
                });
            }
        });
    });

    // Get elements by role
    interactiveRoles.forEach(role => {
        document.querySelectorAll(`[role="${role}"]`).forEach(el => {
            if (isVisible(el) && !elements.find(e => e.selector === getSelector(el))) {
                const rect = el.getBoundingClientRect();
                elements.push({
                    tag: el.tagName.toLowerCase(),
                    selector: getSelector(el),
                    id: el.id || null,
                    class: el.className || null,
                    text: el.textContent?.trim().substring(0, 100) || null,
                    href: el.href || null,
                    aria_label: el.getAttribute('aria-label') || null,
                    placeholder: el.placeholder || null,
                    value: el.value || null,
                    input_type: el.type || null,
                    visible: true,
                    x: Math.round(rect.x),
                    y: Math.round(rect.y),
                    width: Math.round(rect.width),
                    height: Math.round(rect.height)
                });
            }
        });
    });

    // Get clickable elements with onclick
    document.querySelectorAll('[onclick]').forEach(el => {
        if (isVisible(el) && !elements.find(e => e.selector === getSelector(el))) {
            const rect = el.getBoundingClientRect();
            elements.push({
                tag: el.tagName.toLowerCase(),
                selector: getSelector(el),
                id: el.id || null,
                class: el.className || null,
                text: el.textContent?.trim().substring(0, 100) || null,
                href: null,
                aria_label: el.getAttribute('aria-label') || null,
                placeholder: null,
                value: null,
                input_type: null,
                visible: true,
                x: Math.round(rect.x),
                y: Math.round(rect.y),
                width: Math.round(rect.width),
                height: Math.round(rect.height)
            });
        }
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

/// Format elements for LLM context (simplified view)
pub fn format_elements_for_llm(elements: &[InteractiveElement]) -> String {
    let mut output = String::new();

    for (i, el) in elements.iter().enumerate() {
        let mut desc = format!("[{}] <{}> ", i, el.tag);

        if let Some(text) = &el.text {
            if !text.is_empty() {
                desc.push_str(&format!("\"{}\" ", text));
            }
        }

        if let Some(href) = &el.href {
            desc.push_str(&format!("href=\"{}\" ", href));
        }

        if let Some(placeholder) = &el.placeholder {
            desc.push_str(&format!("placeholder=\"{}\" ", placeholder));
        }

        if let Some(aria_label) = &el.aria_label {
            desc.push_str(&format!("aria-label=\"{}\" ", aria_label));
        }

        desc.push_str(&format!("selector=\"{}\"", el.selector));

        output.push_str(&desc);
        output.push('\n');
    }

    output
}
