use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Backend node ID - stable identifier from CDP
pub type BackendNodeId = i64;

/// Sequential index for LLM interaction (1-based)
pub type ElementIndex = i32;

/// Bounding rectangle
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DOMRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl DOMRect {
    pub fn default_viewport() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        }
    }

    pub fn contains_point(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    pub fn intersects(&self, other: &DOMRect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    pub fn intersection_area(&self, other: &DOMRect) -> f64 {
        let x_overlap = (self.x + self.width).min(other.x + other.width) - self.x.max(other.x);
        let y_overlap = (self.y + self.height).min(other.y + other.height) - self.y.max(other.y);
        if x_overlap > 0.0 && y_overlap > 0.0 {
            x_overlap * y_overlap
        } else {
            0.0
        }
    }

    pub fn area(&self) -> f64 {
        self.width * self.height
    }
}

/// Option in a select dropdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub text: String,
    pub selected: bool,
}

/// Accessibility node data from AX tree
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AXNodeData {
    pub role: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    pub disabled: Option<bool>,
    pub expanded: Option<bool>,
    pub focusable: bool,
    pub focused: bool,
    pub required: bool,
    pub readonly: bool,
}

/// Layout/snapshot data for an element
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayoutData {
    pub bounds: DOMRect,
    pub paint_order: Option<i64>,
    pub is_stacking_context: bool,
    pub is_clickable: bool,
    pub cursor_style: Option<String>,
    pub display: Option<String>,
    pub visibility: Option<String>,
    pub opacity: Option<String>,
    pub pointer_events: Option<String>,
}

impl LayoutData {
    pub fn is_visible(&self) -> bool {
        // Check display
        if let Some(display) = &self.display {
            if display == "none" {
                return false;
            }
        }
        // Check visibility
        if let Some(visibility) = &self.visibility {
            if visibility == "hidden" {
                return false;
            }
        }
        // Check opacity
        if let Some(opacity) = &self.opacity {
            if opacity == "0" {
                return false;
            }
        }
        // Check pointer-events: none (element cannot receive clicks)
        if let Some(ref pe) = self.pointer_events {
            if pe == "none" {
                return false;
            }
        }
        // Check bounds
        self.bounds.width > 0.0 && self.bounds.height > 0.0
    }
}

/// Enhanced DOM tree node combining DOM, AX, and layout data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedDOMNode {
    pub backend_node_id: BackendNodeId,
    pub node_id: i64,
    pub node_type: i32,
    pub tag_name: String,
    pub attributes: HashMap<String, String>,
    pub text_content: Option<String>,
    pub ax_data: Option<AXNodeData>,
    pub layout: Option<LayoutData>,
    pub shadow_root_type: Option<String>,
    pub frame_id: Option<String>,
    pub content_document_backend_id: Option<BackendNodeId>,
    pub children: Vec<EnhancedDOMNode>,
    pub is_interactive: bool,
    pub is_visible: bool,
    pub is_obscured: bool,
    pub in_shadow_dom: bool,
}

impl Default for EnhancedDOMNode {
    fn default() -> Self {
        Self {
            backend_node_id: 0,
            node_id: 0,
            node_type: 1, // ELEMENT_NODE
            tag_name: String::new(),
            attributes: HashMap::new(),
            text_content: None,
            ax_data: None,
            layout: None,
            shadow_root_type: None,
            frame_id: None,
            content_document_backend_id: None,
            children: Vec::new(),
            is_interactive: false,
            is_visible: true,
            is_obscured: false,
            in_shadow_dom: false,
        }
    }
}

impl EnhancedDOMNode {
    /// Check if this node is an interactive element
    pub fn compute_interactivity(&self) -> bool {
        let interactive_tags = [
            "a", "button", "input", "select", "textarea", "label", "details", "summary",
        ];

        let interactive_roles = [
            "button",
            "link",
            "checkbox",
            "radio",
            "textbox",
            "searchbox",
            "tab",
            "menuitem",
            "option",
            "switch",
            "slider",
            "combobox",
            "listbox",
        ];

        let tag = self.tag_name.to_lowercase();

        // Check tag
        if interactive_tags.contains(&tag.as_str()) {
            return true;
        }

        // Check role attribute
        if let Some(role) = self.attributes.get("role") {
            if interactive_roles.contains(&role.as_str()) {
                return true;
            }
        }

        // Check AX role
        if let Some(ax) = &self.ax_data {
            if let Some(role) = &ax.role {
                let role_lower = role.to_lowercase();
                if interactive_roles.contains(&role_lower.as_str()) {
                    return true;
                }
            }
        }

        // Check for onclick, tabindex
        if self.attributes.contains_key("onclick") {
            return true;
        }
        if self.attributes.get("tabindex").map(|v| v != "-1").unwrap_or(false) {
            return true;
        }
        if self.attributes.get("contenteditable") == Some(&"true".to_string()) {
            return true;
        }

        false
    }
}

/// Simplified element for LLM serialization and tool operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SimplifiedElement {
    pub index: ElementIndex,
    pub backend_node_id: BackendNodeId,
    pub tag: String,
    pub role: Option<String>,
    pub name: Option<String>,
    pub text: Option<String>,
    pub input_type: Option<String>,
    pub value: Option<String>,
    pub placeholder: Option<String>,
    pub aria_label: Option<String>,
    pub title: Option<String>,
    pub href: Option<String>,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    pub disabled: Option<bool>,
    pub required: Option<bool>,
    pub readonly: Option<bool>,
    pub bounds: DOMRect,
    pub in_shadow_dom: bool,
    pub frame_id: Option<String>,
    /// Options for select elements
    pub select_options: Option<Vec<SelectOption>>,
}

/// Map from index to element for tool operations
#[derive(Debug, Clone, Default)]
pub struct SelectorMap {
    pub index_to_backend_id: HashMap<ElementIndex, BackendNodeId>,
    pub backend_id_to_element: HashMap<BackendNodeId, SimplifiedElement>,
    pub ordered_elements: Vec<SimplifiedElement>,
}

impl SelectorMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_backend_id(&self, index: ElementIndex) -> Option<BackendNodeId> {
        self.index_to_backend_id.get(&index).copied()
    }

    pub fn get_element_by_index(&self, index: ElementIndex) -> Option<&SimplifiedElement> {
        let backend_id = self.index_to_backend_id.get(&index)?;
        self.backend_id_to_element.get(backend_id)
    }

    pub fn get_element_by_backend_id(&self, backend_id: BackendNodeId) -> Option<&SimplifiedElement> {
        self.backend_id_to_element.get(&backend_id)
    }

    pub fn len(&self) -> usize {
        self.ordered_elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ordered_elements.is_empty()
    }
}

/// Result of DOM extraction
#[derive(Debug, Clone)]
pub struct DOMExtractionResult {
    pub selector_map: SelectorMap,
    pub llm_representation: String,
    pub viewport: DOMRect,
    pub url: String,
    pub title: String,
}

impl Default for DOMExtractionResult {
    fn default() -> Self {
        Self {
            selector_map: SelectorMap::new(),
            llm_representation: String::new(),
            viewport: DOMRect::default_viewport(),
            url: String::new(),
            title: String::new(),
        }
    }
}

/// Raw CDP response data before processing
#[derive(Debug)]
pub struct RawCDPTrees {
    pub dom_root: serde_json::Value,
    pub snapshot: Option<serde_json::Value>,
    pub ax_nodes: Option<Vec<serde_json::Value>>,
    pub viewport: DOMRect,
}
