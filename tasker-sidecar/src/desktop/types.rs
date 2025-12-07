use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Stable identifier for OS UI elements (platform-specific)
/// - Windows: Runtime ID as string
/// - macOS: AXUIElement path
/// - Linux: AT-SPI2 accessible path
pub type OSElementId = String;

/// 1-based index for LLM interaction (e.g., [1], [2], [3])
pub type OSElementIndex = i32;

/// Bounding rectangle for OS elements
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OSRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl OSRect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    pub fn is_visible(&self) -> bool {
        self.width > 0.0 && self.height > 0.0
    }
}

/// Window information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    pub process_name: String,
    pub process_id: u32,
    pub bounds: OSRect,
    pub is_focused: bool,
    pub is_minimized: bool,
    pub is_visible: bool,
}

impl WindowInfo {
    pub fn display_name(&self) -> String {
        if self.title.is_empty() {
            self.process_name.clone()
        } else {
            format!("{} - {}", self.title, self.process_name)
        }
    }
}

/// OS UI element (analogous to SimplifiedElement for browser)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OSElement {
    /// Platform-specific stable identifier
    pub id: OSElementId,
    /// 1-based index for LLM interaction
    pub index: OSElementIndex,
    /// Control type: Button, TextField, MenuItem, CheckBox, etc.
    pub control_type: String,
    /// Accessible name (displayed text or label)
    pub name: Option<String>,
    /// Current value (for text fields, sliders, etc.)
    pub value: Option<String>,
    /// Accessible description or help text
    pub description: Option<String>,
    /// Bounding rectangle in screen coordinates
    pub bounds: OSRect,
    /// Whether the element is enabled
    pub is_enabled: bool,
    /// Whether the element can receive focus
    pub is_focusable: bool,
    /// Whether the element currently has focus
    pub is_focused: bool,
    /// Whether the element can be edited (text input)
    pub is_editable: bool,
    /// Whether the element is checked (for checkboxes, radio buttons)
    pub is_checked: Option<bool>,
    /// Whether the element is selected (for list items, tabs)
    pub is_selected: Option<bool>,
    /// Whether the element is expanded (for tree items, menus)
    pub is_expanded: Option<bool>,
    /// Window ID this element belongs to
    pub window_id: String,
    /// Automation ID (Windows) or identifier (macOS)
    pub automation_id: Option<String>,
    /// ARIA-like role if available
    pub role: Option<String>,
    /// Supported interaction patterns
    pub patterns: Vec<String>,
    /// Keyboard shortcut if available
    pub accelerator_key: Option<String>,
    /// Access key (mnemonic) if available
    pub access_key: Option<String>,
}

impl OSElement {
    /// Create a new element with minimal required fields
    pub fn new(id: OSElementId, control_type: String, window_id: String) -> Self {
        Self {
            id,
            index: 0, // Will be assigned later
            control_type,
            name: None,
            value: None,
            description: None,
            bounds: OSRect::default(),
            is_enabled: true,
            is_focusable: false,
            is_focused: false,
            is_editable: false,
            is_checked: None,
            is_selected: None,
            is_expanded: None,
            window_id,
            automation_id: None,
            role: None,
            patterns: Vec::new(),
            accelerator_key: None,
            access_key: None,
        }
    }

    /// Get display text for this element
    pub fn display_text(&self) -> String {
        self.name
            .clone()
            .or_else(|| self.value.clone())
            .or_else(|| self.description.clone())
            .unwrap_or_else(|| self.control_type.clone())
    }

    /// Check if this element is interactive
    pub fn is_interactive(&self) -> bool {
        // Interactive control types
        let interactive_types = [
            "Button",
            "CheckBox",
            "RadioButton",
            "ComboBox",
            "Edit",
            "Text", // Text fields
            "TextField",
            "TextBox",
            "Hyperlink",
            "Link",
            "ListItem",
            "MenuItem",
            "Tab",
            "TabItem",
            "TreeItem",
            "Slider",
            "SpinButton",
            "ScrollBar",
            "ToggleButton",
            "SplitButton",
            "MenuBar",
            "Menu",
        ];

        // Check if control type is interactive
        let type_is_interactive = interactive_types
            .iter()
            .any(|t| self.control_type.eq_ignore_ascii_case(t));

        // Also interactive if has certain patterns
        let has_interactive_pattern = self.patterns.iter().any(|p| {
            matches!(
                p.as_str(),
                "Invoke" | "Toggle" | "SelectionItem" | "ExpandCollapse" | "Value" | "RangeValue"
            )
        });

        (type_is_interactive || has_interactive_pattern || self.is_focusable) && self.is_enabled
    }
}

/// Map from index to OS element (analogous to SelectorMap for browser)
#[derive(Debug, Clone, Default)]
pub struct OSElementMap {
    pub index_to_id: HashMap<OSElementIndex, OSElementId>,
    pub id_to_element: HashMap<OSElementId, OSElement>,
    pub ordered_elements: Vec<OSElement>,
}

impl OSElementMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build element map from a list of elements
    /// Assigns indices 1, 2, 3... to elements sorted by position
    pub fn from_elements(mut elements: Vec<OSElement>) -> Self {
        // Sort by position (top to bottom, left to right)
        elements.sort_by(|a, b| {
            let ay = a.bounds.y as i64;
            let by = b.bounds.y as i64;
            // Group elements within 20px vertically as same "row"
            if (ay - by).abs() > 20 {
                ay.cmp(&by)
            } else {
                (a.bounds.x as i64).cmp(&(b.bounds.x as i64))
            }
        });

        let mut map = Self::new();
        for (i, mut elem) in elements.into_iter().enumerate() {
            let index = (i + 1) as OSElementIndex;
            elem.index = index;
            map.index_to_id.insert(index, elem.id.clone());
            map.id_to_element.insert(elem.id.clone(), elem.clone());
            map.ordered_elements.push(elem);
        }
        map
    }

    /// Get element ID by index
    pub fn get_id(&self, index: OSElementIndex) -> Option<&OSElementId> {
        self.index_to_id.get(&index)
    }

    /// Get element by index
    pub fn get_element_by_index(&self, index: OSElementIndex) -> Option<&OSElement> {
        let id = self.index_to_id.get(&index)?;
        self.id_to_element.get(id)
    }

    /// Get element by ID
    pub fn get_element_by_id(&self, id: &OSElementId) -> Option<&OSElement> {
        self.id_to_element.get(id)
    }

    /// Get number of elements
    pub fn len(&self) -> usize {
        self.ordered_elements.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.ordered_elements.is_empty()
    }
}

/// Result of OS UI extraction (analogous to DOMExtractionResult)
#[derive(Debug, Clone)]
pub struct OSExtractionResult {
    /// Map of indexed elements
    pub element_map: OSElementMap,
    /// Human-readable representation for LLM
    pub llm_representation: String,
    /// Information about the extracted window
    pub window: WindowInfo,
    /// Screenshot as base64 PNG (optional)
    pub screenshot_base64: Option<String>,
}

/// Automation mode for unified agent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AutomationMode {
    /// Only browser tools available
    BrowserOnly,
    /// Only OS/desktop tools available
    OSOnly,
    /// Both browser and OS tools available
    #[default]
    Unified,
}

impl AutomationMode {
    pub fn includes_browser(&self) -> bool {
        matches!(self, Self::BrowserOnly | Self::Unified)
    }

    pub fn includes_os(&self) -> bool {
        matches!(self, Self::OSOnly | Self::Unified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_rect_center() {
        let rect = OSRect::new(100.0, 200.0, 50.0, 30.0);
        let (cx, cy) = rect.center();
        assert_eq!(cx, 125.0);
        assert_eq!(cy, 215.0);
    }

    #[test]
    fn test_element_map_indexing() {
        let elements = vec![
            {
                let mut e = OSElement::new("id1".to_string(), "Button".to_string(), "win1".to_string());
                e.bounds = OSRect::new(100.0, 50.0, 80.0, 30.0);
                e.name = Some("OK".to_string());
                e
            },
            {
                let mut e = OSElement::new("id2".to_string(), "Button".to_string(), "win1".to_string());
                e.bounds = OSRect::new(200.0, 50.0, 80.0, 30.0);
                e.name = Some("Cancel".to_string());
                e
            },
        ];

        let map = OSElementMap::from_elements(elements);

        assert_eq!(map.len(), 2);
        assert_eq!(map.get_element_by_index(1).unwrap().name, Some("OK".to_string()));
        assert_eq!(map.get_element_by_index(2).unwrap().name, Some("Cancel".to_string()));
    }
}
