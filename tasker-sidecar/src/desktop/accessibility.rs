use anyhow::Result;
use std::cmp::Ordering;

/// A rectangle in screen coordinates
#[derive(Debug, Clone, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn is_valid(&self) -> bool {
        self.width > 0.0 && self.height > 0.0
    }
}

/// An interactive element extracted from the accessibility tree
#[derive(Debug, Clone)]
pub struct DesktopElement {
    /// 1-based index, assigned after sorting
    pub index: usize,
    /// Control type: "Button", "TextBox", "Link", "MenuItem", etc.
    pub control_type: String,
    /// Human-readable label
    pub name: String,
    /// Bounding rectangle in screen coordinates
    pub bounds: Rect,
    /// Whether this element currently has keyboard focus
    pub is_focused: bool,
    /// Current value (for text inputs, checkboxes, etc.)
    pub value: Option<String>,
    /// Whether the element is enabled for interaction
    pub is_enabled: bool,
}

impl DesktopElement {
    /// Format as a single line for LLM display
    pub fn to_display_string(&self) -> String {
        let mut parts = vec![format!(
            "[{}] {} \"{}\" at ({}, {})",
            self.index,
            self.control_type,
            self.name,
            self.bounds.x as i32,
            self.bounds.y as i32,
        )];

        if let Some(ref val) = self.value {
            if !val.is_empty() {
                let display_val: String = val.chars().take(50).collect();
                parts.push(format!("value=\"{}\"", display_val));
            }
        }

        if self.is_focused {
            parts.push("[focused]".to_string());
        }

        if !self.is_enabled {
            parts.push("[disabled]".to_string());
        }

        parts.join(" ")
    }
}

/// Map from element index to element data (for click_element lookups)
#[derive(Debug, Default)]
pub struct ElementMap {
    elements: Vec<DesktopElement>,
}

impl ElementMap {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
        }
    }

    pub fn from_elements(elements: &[DesktopElement]) -> Self {
        Self {
            elements: elements.to_vec(),
        }
    }

    /// Get element by 1-based index
    pub fn get(&self, index: usize) -> Option<&DesktopElement> {
        self.elements.iter().find(|e| e.index == index)
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

/// Trait for platform-specific accessibility tree extraction
pub trait AccessibilityProvider: Send + Sync {
    fn extract_interactive_elements(&self) -> Result<Vec<DesktopElement>>;
}

// =============================================================================
// Windows implementation using UI Automation
// =============================================================================

#[cfg(target_os = "windows")]
pub struct WindowsAccessibility;

#[cfg(target_os = "windows")]
impl WindowsAccessibility {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

#[cfg(target_os = "windows")]
impl AccessibilityProvider for WindowsAccessibility {
    fn extract_interactive_elements(&self) -> Result<Vec<DesktopElement>> {
        use uiautomation::UIAutomation;

        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("Failed to init UIAutomation: {}", e))?;
        let root = automation
            .get_root_element()
            .map_err(|e| anyhow::anyhow!("Failed to get root element: {}", e))?;

        // Get the focused window to scope our search
        let focused = automation.get_focused_element().ok();
        let search_root = if let Some(ref _focused_el) = focused {
            // Try to get the top-level window containing the focused element
            match automation.get_focused_element() {
                Ok(el) => {
                    // Walk up to find the top-level window
                    let mut current = el;
                    loop {
                        match automation.create_tree_walker().and_then(|w| w.get_parent(&current)) {
                            Ok(parent) => {
                                // Check if parent is the root
                                let parent_rect = parent.get_bounding_rectangle().unwrap_or_default();
                                let root_rect = root.get_bounding_rectangle().unwrap_or_default();
                                if parent_rect == root_rect {
                                    break;
                                }
                                current = parent;
                            }
                            Err(_) => break,
                        }
                    }
                    current
                }
                Err(_) => root,
            }
        } else {
            root
        };

        let mut elements = Vec::new();
        walk_uia_tree(&automation, &search_root, &mut elements, 0)?;

        // Sort by position: top-to-bottom, left-to-right
        elements.sort_by(|a, b| {
            let y_cmp = a.bounds.y.partial_cmp(&b.bounds.y).unwrap_or(Ordering::Equal);
            if y_cmp == Ordering::Equal {
                a.bounds.x.partial_cmp(&b.bounds.x).unwrap_or(Ordering::Equal)
            } else {
                y_cmp
            }
        });

        // Assign 1-based indices and limit count
        let max_elements = 100;
        elements.truncate(max_elements);
        for (i, elem) in elements.iter_mut().enumerate() {
            elem.index = i + 1;
        }

        Ok(elements)
    }
}

#[cfg(target_os = "windows")]
fn walk_uia_tree(
    automation: &uiautomation::UIAutomation,
    element: &uiautomation::UIElement,
    results: &mut Vec<DesktopElement>,
    depth: usize,
) -> Result<()> {
    // Limit recursion depth to avoid infinite loops
    if depth > 15 || results.len() >= 150 {
        return Ok(());
    }

    // Check if this element is interactive
    if let Some(desktop_elem) = try_extract_element(element) {
        results.push(desktop_elem);
    }

    // Walk children
    let walker = automation
        .create_tree_walker()
        .map_err(|e| anyhow::anyhow!("Failed to create tree walker: {}", e))?;

    if let Ok(child) = walker.get_first_child(element) {
        walk_uia_tree(automation, &child, results, depth + 1)?;

        let mut current = child;
        while let Ok(sibling) = walker.get_next_sibling(&current) {
            walk_uia_tree(automation, &sibling, results, depth + 1)?;
            current = sibling;
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn try_extract_element(element: &uiautomation::UIElement) -> Option<DesktopElement> {
    use uiautomation::types::UIProperty;

    // Get control type
    let control_type = element.get_control_type().ok()?;

    // Filter to interactive control types
    let type_name = match control_type {
        uiautomation::controls::ControlType::Button => "Button",
        uiautomation::controls::ControlType::CheckBox => "CheckBox",
        uiautomation::controls::ControlType::ComboBox => "ComboBox",
        uiautomation::controls::ControlType::Edit => "TextBox",
        uiautomation::controls::ControlType::Hyperlink => "Link",
        uiautomation::controls::ControlType::ListItem => "ListItem",
        uiautomation::controls::ControlType::MenuItem => "MenuItem",
        uiautomation::controls::ControlType::RadioButton => "RadioButton",
        uiautomation::controls::ControlType::Slider => "Slider",
        uiautomation::controls::ControlType::Spinner => "Spinner",
        uiautomation::controls::ControlType::Tab => "Tab",
        uiautomation::controls::ControlType::TabItem => "TabItem",
        uiautomation::controls::ControlType::TreeItem => "TreeItem",
        uiautomation::controls::ControlType::SplitButton => "SplitButton",
        uiautomation::controls::ControlType::Document => "Document",
        _ => return None, // Skip non-interactive types
    };

    // Check if element is enabled
    let is_enabled = element
        .get_property_value(UIProperty::IsEnabled)
        .ok()
        .and_then(|v| v.try_into().ok())
        .unwrap_or(true);

    // Get bounding rectangle
    let rect = element.get_bounding_rectangle().ok()?;
    let bounds = Rect {
        x: rect.get_left() as f64,
        y: rect.get_top() as f64,
        width: rect.get_width() as f64,
        height: rect.get_height() as f64,
    };

    // Skip elements with invalid/offscreen bounds
    if !bounds.is_valid() || bounds.x < -10.0 || bounds.y < -10.0 {
        return None;
    }

    // Skip very tiny elements (likely invisible)
    if bounds.width < 3.0 || bounds.height < 3.0 {
        return None;
    }

    // Get name
    let name = element.get_name().unwrap_or_default();

    // Skip elements with no name and no value (usually not useful)
    let value = element
        .get_property_value(UIProperty::ValueValue)
        .ok()
        .and_then(|v| {
            let s: String = v.try_into().ok()?;
            if s.is_empty() { None } else { Some(s) }
        });

    // For some control types, an empty name is OK if there's a value
    if name.is_empty() && value.is_none() && type_name != "TextBox" && type_name != "Document" {
        return None;
    }

    // Check if focused
    let is_focused = element
        .get_property_value(UIProperty::HasKeyboardFocus)
        .ok()
        .and_then(|v| v.try_into().ok())
        .unwrap_or(false);

    // Check if offscreen
    let is_offscreen = element
        .get_property_value(UIProperty::IsOffscreen)
        .ok()
        .and_then(|v| v.try_into().ok())
        .unwrap_or(false);

    if is_offscreen {
        return None;
    }

    Some(DesktopElement {
        index: 0, // Assigned later after sorting
        control_type: type_name.to_string(),
        name,
        bounds,
        is_focused,
        value,
        is_enabled,
    })
}

// =============================================================================
// Fallback implementation for non-Windows platforms
// =============================================================================

#[cfg(not(target_os = "windows"))]
pub struct FallbackAccessibility;

#[cfg(not(target_os = "windows"))]
impl FallbackAccessibility {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

#[cfg(not(target_os = "windows"))]
impl AccessibilityProvider for FallbackAccessibility {
    fn extract_interactive_elements(&self) -> Result<Vec<DesktopElement>> {
        // No accessibility extraction on non-Windows — agent uses coordinate-based tools only
        Ok(vec![])
    }
}

/// Format a list of elements for LLM consumption
pub fn format_elements(elements: &[DesktopElement]) -> String {
    if elements.is_empty() {
        return "(No interactive elements detected)".to_string();
    }

    elements
        .iter()
        .map(|e| e.to_display_string())
        .collect::<Vec<_>>()
        .join("\n")
}
