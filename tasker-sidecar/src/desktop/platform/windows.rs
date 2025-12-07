#![cfg(target_os = "windows")]

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use windows::{
    core::{Interface, BSTR},
    Win32::{
        Foundation::{BOOL, HWND, LPARAM, RECT},
        System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
        UI::{
            Accessibility::{
                CUIAutomation, IUIAutomation, IUIAutomationElement,
                IUIAutomationInvokePattern, IUIAutomationSelectionItemPattern,
                IUIAutomationTogglePattern, IUIAutomationValuePattern,
                TreeScope_Children, TreeScope_Subtree,
                UIA_ButtonControlTypeId, UIA_CheckBoxControlTypeId, UIA_ComboBoxControlTypeId,
                UIA_EditControlTypeId, UIA_HyperlinkControlTypeId, UIA_InvokePatternId,
                UIA_ListControlTypeId, UIA_ListItemControlTypeId, UIA_MenuBarControlTypeId,
                UIA_MenuControlTypeId, UIA_MenuItemControlTypeId, UIA_PaneControlTypeId,
                UIA_RadioButtonControlTypeId, UIA_ScrollBarControlTypeId,
                UIA_SelectionItemPatternId, UIA_SliderControlTypeId, UIA_SpinnerControlTypeId,
                UIA_SplitButtonControlTypeId, UIA_TabControlTypeId, UIA_TabItemControlTypeId,
                UIA_TextControlTypeId, UIA_TogglePatternId, UIA_ToolBarControlTypeId,
                UIA_TreeControlTypeId, UIA_TreeItemControlTypeId, UIA_ValuePatternId,
                UIA_WindowControlTypeId,
            },
            WindowsAndMessaging::{
                EnumWindows, GetForegroundWindow, GetWindowRect, GetWindowTextW,
                GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow,
                ShowWindow, SW_RESTORE,
            },
        },
    },
};

use super::AccessibilityProvider;
use crate::desktop::types::{OSElement, OSElementId, OSRect, WindowInfo};

/// Windows UI Automation provider
///
/// Note: COM objects are created fresh in each method call to ensure thread safety.
pub struct WindowsAccessibility;

impl WindowsAccessibility {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Create a new UI Automation instance (must be called on the thread that will use it)
    fn create_automation() -> Result<IUIAutomation> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)
                .map_err(|e| anyhow!("Failed to create UIAutomation: {}", e))
        }
    }

    /// Get element control type as string
    fn get_control_type_name(control_type_id: i32) -> &'static str {
        match control_type_id {
            x if x == UIA_ButtonControlTypeId.0 => "Button",
            x if x == UIA_CheckBoxControlTypeId.0 => "CheckBox",
            x if x == UIA_ComboBoxControlTypeId.0 => "ComboBox",
            x if x == UIA_EditControlTypeId.0 => "Edit",
            x if x == UIA_HyperlinkControlTypeId.0 => "Hyperlink",
            x if x == UIA_ListControlTypeId.0 => "List",
            x if x == UIA_ListItemControlTypeId.0 => "ListItem",
            x if x == UIA_MenuControlTypeId.0 => "Menu",
            x if x == UIA_MenuBarControlTypeId.0 => "MenuBar",
            x if x == UIA_MenuItemControlTypeId.0 => "MenuItem",
            x if x == UIA_PaneControlTypeId.0 => "Pane",
            x if x == UIA_RadioButtonControlTypeId.0 => "RadioButton",
            x if x == UIA_ScrollBarControlTypeId.0 => "ScrollBar",
            x if x == UIA_SliderControlTypeId.0 => "Slider",
            x if x == UIA_SpinnerControlTypeId.0 => "Spinner",
            x if x == UIA_SplitButtonControlTypeId.0 => "SplitButton",
            x if x == UIA_TabControlTypeId.0 => "Tab",
            x if x == UIA_TabItemControlTypeId.0 => "TabItem",
            x if x == UIA_TextControlTypeId.0 => "Text",
            x if x == UIA_ToolBarControlTypeId.0 => "ToolBar",
            x if x == UIA_TreeControlTypeId.0 => "Tree",
            x if x == UIA_TreeItemControlTypeId.0 => "TreeItem",
            x if x == UIA_WindowControlTypeId.0 => "Window",
            _ => "Unknown",
        }
    }

    /// Check if control type is interactive
    fn is_interactive_control_type(control_type_id: i32) -> bool {
        control_type_id == UIA_ButtonControlTypeId.0
            || control_type_id == UIA_CheckBoxControlTypeId.0
            || control_type_id == UIA_ComboBoxControlTypeId.0
            || control_type_id == UIA_EditControlTypeId.0
            || control_type_id == UIA_HyperlinkControlTypeId.0
            || control_type_id == UIA_ListItemControlTypeId.0
            || control_type_id == UIA_MenuItemControlTypeId.0
            || control_type_id == UIA_RadioButtonControlTypeId.0
            || control_type_id == UIA_SliderControlTypeId.0
            || control_type_id == UIA_SpinnerControlTypeId.0
            || control_type_id == UIA_SplitButtonControlTypeId.0
            || control_type_id == UIA_TabItemControlTypeId.0
            || control_type_id == UIA_TreeItemControlTypeId.0
    }

    /// Get element bounding rect
    fn get_element_rect(element: &IUIAutomationElement) -> Result<OSRect> {
        unsafe {
            let rect = element
                .CurrentBoundingRectangle()
                .map_err(|e| anyhow!("Failed to get bounding rect: {}", e))?;
            Ok(OSRect::new(
                rect.left as f64,
                rect.top as f64,
                (rect.right - rect.left) as f64,
                (rect.bottom - rect.top) as f64,
            ))
        }
    }

    /// Get element value if it has ValuePattern
    fn get_element_value(element: &IUIAutomationElement) -> Option<String> {
        unsafe {
            element
                .GetCurrentPattern(UIA_ValuePatternId)
                .ok()
                .and_then(|pattern| {
                    pattern
                        .cast::<IUIAutomationValuePattern>()
                        .ok()
                        .and_then(|vp| vp.CurrentValue().ok().map(|s| s.to_string()))
                })
        }
    }

    /// Convert UIA element to OSElement
    fn element_to_os_element(
        element: &IUIAutomationElement,
        window_id: &str,
    ) -> Result<OSElement> {
        unsafe {
            // Generate unique ID from runtime ID or UUID
            let runtime_id = element
                .GetRuntimeId()
                .ok()
                .and_then(|arr| {
                    // SAFEARRAY handling - just use first few bytes as identifier
                    let bounds = (*arr).rgsabound[0];
                    let data = (*arr).pvData as *const i32;
                    let mut parts = Vec::new();
                    for i in 0..bounds.cElements.min(10) {
                        parts.push((*data.add(i as usize)).to_string());
                    }
                    if parts.is_empty() {
                        None
                    } else {
                        Some(parts.join("."))
                    }
                })
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let control_type = element.CurrentControlType().map(|c| c.0).unwrap_or(0);
            let control_type_name = Self::get_control_type_name(control_type).to_string();

            let name = element
                .CurrentName()
                .ok()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());

            let value = Self::get_element_value(element);

            let automation_id = element
                .CurrentAutomationId()
                .ok()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());

            let is_enabled = element.CurrentIsEnabled().map(|b| b.as_bool()).unwrap_or(true);

            let is_keyboard_focusable = element
                .CurrentIsKeyboardFocusable()
                .map(|b| b.as_bool())
                .unwrap_or(false);

            let has_keyboard_focus = element
                .CurrentHasKeyboardFocus()
                .map(|b| b.as_bool())
                .unwrap_or(false);

            let bounds = Self::get_element_rect(element)?;

            // Get patterns for interaction capabilities
            let mut patterns = Vec::new();
            if element.GetCurrentPattern(UIA_InvokePatternId).is_ok() {
                patterns.push("Invoke".to_string());
            }
            if element.GetCurrentPattern(UIA_TogglePatternId).is_ok() {
                patterns.push("Toggle".to_string());
            }
            if element.GetCurrentPattern(UIA_ValuePatternId).is_ok() {
                patterns.push("Value".to_string());
            }
            if element.GetCurrentPattern(UIA_SelectionItemPatternId).is_ok() {
                patterns.push("SelectionItem".to_string());
            }

            let is_editable = patterns.contains(&"Value".to_string());

            let accelerator_key = element
                .CurrentAcceleratorKey()
                .ok()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());

            let access_key = element
                .CurrentAccessKey()
                .ok()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());

            Ok(OSElement {
                id: runtime_id,
                index: 0,
                control_type: control_type_name,
                name,
                value,
                description: None,
                bounds,
                is_enabled,
                is_focusable: is_keyboard_focusable,
                is_focused: has_keyboard_focus,
                is_editable,
                is_checked: None,
                is_selected: None,
                is_expanded: None,
                window_id: window_id.to_string(),
                automation_id,
                role: None,
                patterns,
                accelerator_key,
                access_key,
            })
        }
    }

    /// Recursively collect interactive elements
    fn collect_interactive_elements(
        automation: &IUIAutomation,
        element: &IUIAutomationElement,
        window_id: &str,
        elements: &mut Vec<OSElement>,
        depth: u32,
        max_depth: u32,
    ) -> Result<()> {
        if depth > max_depth {
            return Ok(());
        }

        unsafe {
            let control_type = element.CurrentControlType().map(|c| c.0).unwrap_or(0);

            if Self::is_interactive_control_type(control_type) {
                if let Ok(rect) = Self::get_element_rect(element) {
                    if rect.is_visible() && rect.width > 5.0 && rect.height > 5.0 {
                        if let Ok(os_elem) = Self::element_to_os_element(element, window_id) {
                            if os_elem.is_enabled {
                                elements.push(os_elem);
                            }
                        }
                    }
                }
            }

            // Get children and recurse
            if let Ok(condition) = automation.CreateTrueCondition() {
                if let Ok(children) = element.FindAll(TreeScope_Children, &condition) {
                    let count = children.Length().unwrap_or(0);
                    for i in 0..count {
                        if let Ok(child) = children.GetElement(i) {
                            let _ = Self::collect_interactive_elements(
                                automation,
                                &child,
                                window_id,
                                elements,
                                depth + 1,
                                max_depth,
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Find element by runtime ID string
    fn find_element_by_id(
        automation: &IUIAutomation,
        root: &IUIAutomationElement,
        id: &str,
    ) -> Result<Option<IUIAutomationElement>> {
        unsafe {
            let condition = automation.CreateTrueCondition()?;
            let elements = root.FindAll(TreeScope_Subtree, &condition)?;

            let count = elements.Length().unwrap_or(0);
            for i in 0..count {
                if let Ok(element) = elements.GetElement(i) {
                    if let Ok(runtime_arr) = element.GetRuntimeId() {
                        let bounds = (*runtime_arr).rgsabound[0];
                        let data = (*runtime_arr).pvData as *const i32;
                        let mut parts = Vec::new();
                        for j in 0..bounds.cElements.min(10) {
                            parts.push((*data.add(j as usize)).to_string());
                        }
                        let element_id = parts.join(".");
                        if element_id == id {
                            return Ok(Some(element));
                        }
                    }
                }
            }

            Ok(None)
        }
    }
}

// WindowsAccessibility is Send + Sync because it holds no state
unsafe impl Send for WindowsAccessibility {}
unsafe impl Sync for WindowsAccessibility {}

#[async_trait]
impl AccessibilityProvider for WindowsAccessibility {
    async fn get_windows(&self) -> Result<Vec<WindowInfo>> {
        tokio::task::spawn_blocking(move || {
            let mut windows = Vec::new();

            unsafe {
                unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
                    let windows = &mut *(lparam.0 as *mut Vec<WindowInfo>);

                    if !IsWindowVisible(hwnd).as_bool() {
                        return BOOL(1);
                    }

                    let mut title_buf = [0u16; 512];
                    let len = GetWindowTextW(hwnd, &mut title_buf);
                    if len == 0 {
                        return BOOL(1);
                    }
                    let title = String::from_utf16_lossy(&title_buf[..len as usize]);

                    if title.trim().is_empty() {
                        return BOOL(1);
                    }

                    let mut rect = RECT::default();
                    if GetWindowRect(hwnd, &mut rect).is_err() {
                        return BOOL(1);
                    }

                    let mut process_id: u32 = 0;
                    GetWindowThreadProcessId(hwnd, Some(&mut process_id));

                    let is_focused = hwnd == GetForegroundWindow();

                    let width = rect.right - rect.left;
                    let height = rect.bottom - rect.top;
                    let is_minimized = width <= 0 || height <= 0;

                    windows.push(WindowInfo {
                        id: (hwnd.0 as usize).to_string(),
                        title: title.clone(),
                        process_name: title,
                        process_id,
                        bounds: OSRect::new(
                            rect.left as f64,
                            rect.top as f64,
                            width as f64,
                            height as f64,
                        ),
                        is_focused,
                        is_minimized,
                        is_visible: true,
                    });

                    BOOL(1)
                }

                let windows_ptr = LPARAM(&mut windows as *mut Vec<WindowInfo> as isize);
                let _ = EnumWindows(Some(enum_callback), windows_ptr);
            }

            Ok(windows)
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    async fn get_focused_window(&self) -> Result<Option<WindowInfo>> {
        tokio::task::spawn_blocking(move || {
            unsafe {
                let hwnd = GetForegroundWindow();
                if hwnd.0 as usize == 0 {
                    return Ok(None);
                }

                let mut title_buf = [0u16; 512];
                let len = GetWindowTextW(hwnd, &mut title_buf);
                let title = if len > 0 {
                    String::from_utf16_lossy(&title_buf[..len as usize])
                } else {
                    String::new()
                };

                let mut rect = RECT::default();
                GetWindowRect(hwnd, &mut rect)?;

                let mut process_id: u32 = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut process_id));

                let width = rect.right - rect.left;
                let height = rect.bottom - rect.top;

                Ok(Some(WindowInfo {
                    id: (hwnd.0 as usize).to_string(),
                    title: title.clone(),
                    process_name: title,
                    process_id,
                    bounds: OSRect::new(
                        rect.left as f64,
                        rect.top as f64,
                        width as f64,
                        height as f64,
                    ),
                    is_focused: true,
                    is_minimized: width <= 0 || height <= 0,
                    is_visible: true,
                }))
            }
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    async fn get_elements(&self, window_id: &str) -> Result<Vec<OSElement>> {
        let window_id = window_id.to_string();

        tokio::task::spawn_blocking(move || {
            let automation = Self::create_automation()?;

            unsafe {
                let hwnd_val: usize = window_id
                    .parse()
                    .map_err(|_| anyhow!("Invalid window ID"))?;
                let hwnd = HWND(hwnd_val as *mut _);

                let root = automation
                    .ElementFromHandle(hwnd)
                    .map_err(|e| anyhow!("Failed to get element from handle: {}", e))?;

                let mut elements = Vec::new();
                Self::collect_interactive_elements(
                    &automation,
                    &root,
                    &window_id,
                    &mut elements,
                    0,
                    15,
                )?;

                Ok(elements)
            }
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    async fn focus_window(&self, window_id: &str) -> Result<()> {
        let window_id = window_id.to_string();

        tokio::task::spawn_blocking(move || {
            unsafe {
                let hwnd_val: usize = window_id
                    .parse()
                    .map_err(|_| anyhow!("Invalid window ID"))?;
                let hwnd = HWND(hwnd_val as *mut _);

                let _ = ShowWindow(hwnd, SW_RESTORE);

                if !SetForegroundWindow(hwnd).as_bool() {
                    return Err(anyhow!("Failed to set foreground window"));
                }

                Ok(())
            }
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    async fn get_element(&self, element_id: &OSElementId) -> Result<Option<OSElement>> {
        let element_id = element_id.clone();

        tokio::task::spawn_blocking(move || {
            let automation = Self::create_automation()?;

            unsafe {
                let root = automation
                    .GetRootElement()
                    .map_err(|e| anyhow!("Failed to get root element: {}", e))?;

                if let Some(element) = Self::find_element_by_id(&automation, &root, &element_id)? {
                    let os_elem = Self::element_to_os_element(&element, "")?;
                    return Ok(Some(os_elem));
                }

                Ok(None)
            }
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    async fn invoke_element(&self, element_id: &OSElementId) -> Result<()> {
        let element_id = element_id.clone();

        tokio::task::spawn_blocking(move || {
            let automation = Self::create_automation()?;

            unsafe {
                let root = automation
                    .GetRootElement()
                    .map_err(|e| anyhow!("Failed to get root element: {}", e))?;

                let element = Self::find_element_by_id(&automation, &root, &element_id)?
                    .ok_or_else(|| anyhow!("Element not found: {}", element_id))?;

                // Try InvokePattern
                if let Ok(pattern) = element.GetCurrentPattern(UIA_InvokePatternId) {
                    if let Ok(invoke) = pattern.cast::<IUIAutomationInvokePattern>() {
                        invoke.Invoke()?;
                        return Ok(());
                    }
                }

                // Try TogglePattern
                if let Ok(pattern) = element.GetCurrentPattern(UIA_TogglePatternId) {
                    if let Ok(toggle) = pattern.cast::<IUIAutomationTogglePattern>() {
                        toggle.Toggle()?;
                        return Ok(());
                    }
                }

                // Try SelectionItemPattern
                if let Ok(pattern) = element.GetCurrentPattern(UIA_SelectionItemPatternId) {
                    if let Ok(selection) = pattern.cast::<IUIAutomationSelectionItemPattern>() {
                        selection.Select()?;
                        return Ok(());
                    }
                }

                Err(anyhow!("Element does not support invoke patterns"))
            }
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    async fn set_element_value(&self, element_id: &OSElementId, value: &str) -> Result<()> {
        let element_id = element_id.clone();
        let value = value.to_string();

        tokio::task::spawn_blocking(move || {
            let automation = Self::create_automation()?;

            unsafe {
                let root = automation
                    .GetRootElement()
                    .map_err(|e| anyhow!("Failed to get root element: {}", e))?;

                let element = Self::find_element_by_id(&automation, &root, &element_id)?
                    .ok_or_else(|| anyhow!("Element not found: {}", element_id))?;

                let pattern = element
                    .GetCurrentPattern(UIA_ValuePatternId)
                    .map_err(|_| anyhow!("Element does not support ValuePattern"))?;

                let value_pattern = pattern
                    .cast::<IUIAutomationValuePattern>()
                    .map_err(|_| anyhow!("Failed to cast to ValuePattern"))?;

                let bstr = BSTR::from(&value);
                value_pattern
                    .SetValue(&bstr)
                    .map_err(|e| anyhow!("Failed to set value: {}", e))?;

                Ok(())
            }
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    async fn expand_element(&self, _element_id: &OSElementId) -> Result<()> {
        Err(anyhow!("Expand not yet implemented"))
    }

    async fn collapse_element(&self, _element_id: &OSElementId) -> Result<()> {
        Err(anyhow!("Collapse not yet implemented"))
    }

    async fn scroll_to_element(&self, _element_id: &OSElementId) -> Result<()> {
        Err(anyhow!("Scroll to element not yet implemented"))
    }

    async fn toggle_element(&self, element_id: &OSElementId) -> Result<()> {
        let element_id = element_id.clone();

        tokio::task::spawn_blocking(move || {
            let automation = Self::create_automation()?;

            unsafe {
                let root = automation
                    .GetRootElement()
                    .map_err(|e| anyhow!("Failed to get root element: {}", e))?;

                let element = Self::find_element_by_id(&automation, &root, &element_id)?
                    .ok_or_else(|| anyhow!("Element not found: {}", element_id))?;

                let pattern = element
                    .GetCurrentPattern(UIA_TogglePatternId)
                    .map_err(|_| anyhow!("Element does not support TogglePattern"))?;

                let toggle = pattern
                    .cast::<IUIAutomationTogglePattern>()
                    .map_err(|_| anyhow!("Failed to cast to TogglePattern"))?;

                toggle.Toggle().map_err(|e| anyhow!("Failed to toggle: {}", e))?;

                Ok(())
            }
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    async fn select_element(&self, element_id: &OSElementId) -> Result<()> {
        let element_id = element_id.clone();

        tokio::task::spawn_blocking(move || {
            let automation = Self::create_automation()?;

            unsafe {
                let root = automation
                    .GetRootElement()
                    .map_err(|e| anyhow!("Failed to get root element: {}", e))?;

                let element = Self::find_element_by_id(&automation, &root, &element_id)?
                    .ok_or_else(|| anyhow!("Element not found: {}", element_id))?;

                let pattern = element
                    .GetCurrentPattern(UIA_SelectionItemPatternId)
                    .map_err(|_| anyhow!("Element does not support SelectionItemPattern"))?;

                let selection = pattern
                    .cast::<IUIAutomationSelectionItemPattern>()
                    .map_err(|_| anyhow!("Failed to cast to SelectionItemPattern"))?;

                selection.Select().map_err(|e| anyhow!("Failed to select: {}", e))?;

                Ok(())
            }
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    async fn get_element_text(&self, element_id: &OSElementId) -> Result<Option<String>> {
        let element_id = element_id.clone();

        tokio::task::spawn_blocking(move || {
            let automation = Self::create_automation()?;

            unsafe {
                let root = automation
                    .GetRootElement()
                    .map_err(|e| anyhow!("Failed to get root element: {}", e))?;

                let element = Self::find_element_by_id(&automation, &root, &element_id)?
                    .ok_or_else(|| anyhow!("Element not found: {}", element_id))?;

                // Try ValuePattern first
                if let Some(value) = Self::get_element_value(&element) {
                    return Ok(Some(value));
                }

                // Try Name property
                if let Ok(name) = element.CurrentName() {
                    let name_str = name.to_string();
                    if !name_str.is_empty() {
                        return Ok(Some(name_str));
                    }
                }

                Ok(None)
            }
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }
}
