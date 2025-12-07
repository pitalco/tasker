//! OS-level automation tools using vision-based coordinate system
//!
//! These tools enable the agent to interact with the entire operating system
//! using a grid-based coordinate system overlaid on screenshots.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::registry::{Tool, ToolContext, ToolDefinition, ToolResult};
use crate::desktop::{DesktopManager, KeyCode, Modifier};

/// Context extension for OS tools
/// This will be added to ToolContext or passed separately
pub struct OsToolContext {
    pub desktop: Arc<Mutex<DesktopManager>>,
}

// ============================================================================
// Screenshot Tools
// ============================================================================

/// Take a screenshot with grid overlay
pub struct OsScreenshotTool {
    pub desktop: Arc<Mutex<DesktopManager>>,
}

#[async_trait]
impl Tool for OsScreenshotTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "os_screenshot".to_string(),
            description: "Take a screenshot of the entire screen with a grid overlay. \
                          The grid helps identify locations using cell references like 'A1', 'B5', etc. \
                          Use this to see the current screen state and identify where to click.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "with_grid": {
                        "type": "boolean",
                        "description": "Whether to include the grid overlay (default: true)",
                        "default": true
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let with_grid = params["with_grid"].as_bool().unwrap_or(true);

        let mut desktop = self.desktop.lock().await;

        if with_grid {
            match desktop.capture_with_grid_base64() {
                Ok((base64, description)) => Ok(ToolResult::success_with_data(
                    format!("Screenshot captured with grid overlay. {}", description),
                    json!({
                        "screenshot": base64,
                        "grid_description": description,
                        "has_grid": true
                    }),
                )),
                Err(e) => Ok(ToolResult::error(format!("Failed to capture screenshot: {}", e))),
            }
        } else {
            match desktop.capture_screen_base64() {
                Ok(base64) => Ok(ToolResult::success_with_data(
                    "Screenshot captured without grid overlay.".to_string(),
                    json!({
                        "screenshot": base64,
                        "has_grid": false
                    }),
                )),
                Err(e) => Ok(ToolResult::error(format!("Failed to capture screenshot: {}", e))),
            }
        }
    }
}

// ============================================================================
// Click Tools
// ============================================================================

/// Click at a grid cell or coordinates
pub struct OsClickTool {
    pub desktop: Arc<Mutex<DesktopManager>>,
}

#[async_trait]
impl Tool for OsClickTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "os_click".to_string(),
            description: "Click at a grid cell (e.g., 'B5') or specific coordinates. \
                          Use grid cells when working with the grid overlay screenshot.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "cell": {
                        "type": "string",
                        "description": "Grid cell to click (e.g., 'A1', 'B5', 'C12'). Takes priority over x/y."
                    },
                    "x": {
                        "type": "integer",
                        "description": "X coordinate (pixels from left). Used if 'cell' is not provided."
                    },
                    "y": {
                        "type": "integer",
                        "description": "Y coordinate (pixels from top). Used if 'cell' is not provided."
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let mut desktop = self.desktop.lock().await;

        if let Some(cell) = params["cell"].as_str() {
            match desktop.click_cell(cell) {
                Ok(()) => Ok(ToolResult::success(format!("Clicked at grid cell {}", cell))),
                Err(e) => Ok(ToolResult::error(format!("Failed to click at {}: {}", cell, e))),
            }
        } else if let (Some(x), Some(y)) = (params["x"].as_i64(), params["y"].as_i64()) {
            match desktop.click_at(x as i32, y as i32) {
                Ok(()) => Ok(ToolResult::success(format!("Clicked at coordinates ({}, {})", x, y))),
                Err(e) => Ok(ToolResult::error(format!("Failed to click at ({}, {}): {}", x, y, e))),
            }
        } else {
            Ok(ToolResult::error("Either 'cell' or both 'x' and 'y' must be provided"))
        }
    }
}

/// Double-click at a grid cell or coordinates
pub struct OsDoubleClickTool {
    pub desktop: Arc<Mutex<DesktopManager>>,
}

#[async_trait]
impl Tool for OsDoubleClickTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "os_double_click".to_string(),
            description: "Double-click at a grid cell or coordinates. Use for opening files, selecting words, etc.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "cell": {
                        "type": "string",
                        "description": "Grid cell to double-click (e.g., 'A1', 'B5')"
                    },
                    "x": {
                        "type": "integer",
                        "description": "X coordinate (if not using cell)"
                    },
                    "y": {
                        "type": "integer",
                        "description": "Y coordinate (if not using cell)"
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let mut desktop = self.desktop.lock().await;

        if let Some(cell) = params["cell"].as_str() {
            match desktop.double_click_cell(cell) {
                Ok(()) => Ok(ToolResult::success(format!("Double-clicked at grid cell {}", cell))),
                Err(e) => Ok(ToolResult::error(format!("Failed to double-click at {}: {}", cell, e))),
            }
        } else if let (Some(x), Some(y)) = (params["x"].as_i64(), params["y"].as_i64()) {
            match desktop.double_click_at(x as i32, y as i32) {
                Ok(()) => Ok(ToolResult::success(format!("Double-clicked at ({}, {})", x, y))),
                Err(e) => Ok(ToolResult::error(format!("Failed to double-click: {}", e))),
            }
        } else {
            Ok(ToolResult::error("Either 'cell' or both 'x' and 'y' must be provided"))
        }
    }
}

/// Right-click at a grid cell or coordinates
pub struct OsRightClickTool {
    pub desktop: Arc<Mutex<DesktopManager>>,
}

#[async_trait]
impl Tool for OsRightClickTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "os_right_click".to_string(),
            description: "Right-click at a grid cell or coordinates. Opens context menus.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "cell": {
                        "type": "string",
                        "description": "Grid cell to right-click"
                    },
                    "x": {
                        "type": "integer",
                        "description": "X coordinate (if not using cell)"
                    },
                    "y": {
                        "type": "integer",
                        "description": "Y coordinate (if not using cell)"
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let mut desktop = self.desktop.lock().await;

        if let Some(cell) = params["cell"].as_str() {
            match desktop.right_click_cell(cell) {
                Ok(()) => Ok(ToolResult::success(format!("Right-clicked at grid cell {}", cell))),
                Err(e) => Ok(ToolResult::error(format!("Failed to right-click at {}: {}", cell, e))),
            }
        } else if let (Some(x), Some(y)) = (params["x"].as_i64(), params["y"].as_i64()) {
            match desktop.right_click_at(x as i32, y as i32) {
                Ok(()) => Ok(ToolResult::success(format!("Right-clicked at ({}, {})", x, y))),
                Err(e) => Ok(ToolResult::error(format!("Failed to right-click: {}", e))),
            }
        } else {
            Ok(ToolResult::error("Either 'cell' or both 'x' and 'y' must be provided"))
        }
    }
}

// ============================================================================
// Mouse Movement Tools
// ============================================================================

/// Move mouse to a location
pub struct OsMoveTool {
    pub desktop: Arc<Mutex<DesktopManager>>,
}

#[async_trait]
impl Tool for OsMoveTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "os_move_mouse".to_string(),
            description: "Move the mouse cursor to a grid cell or coordinates without clicking.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "cell": {
                        "type": "string",
                        "description": "Grid cell to move to"
                    },
                    "x": {
                        "type": "integer",
                        "description": "X coordinate (if not using cell)"
                    },
                    "y": {
                        "type": "integer",
                        "description": "Y coordinate (if not using cell)"
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let mut desktop = self.desktop.lock().await;

        if let Some(cell) = params["cell"].as_str() {
            match desktop.move_to_cell(cell) {
                Ok(()) => Ok(ToolResult::success(format!("Moved mouse to grid cell {}", cell))),
                Err(e) => Ok(ToolResult::error(format!("Failed to move to {}: {}", cell, e))),
            }
        } else if let (Some(x), Some(y)) = (params["x"].as_i64(), params["y"].as_i64()) {
            match desktop.move_mouse(x as i32, y as i32) {
                Ok(()) => Ok(ToolResult::success(format!("Moved mouse to ({}, {})", x, y))),
                Err(e) => Ok(ToolResult::error(format!("Failed to move mouse: {}", e))),
            }
        } else {
            Ok(ToolResult::error("Either 'cell' or both 'x' and 'y' must be provided"))
        }
    }
}

/// Scroll at a location
pub struct OsScrollTool {
    pub desktop: Arc<Mutex<DesktopManager>>,
}

#[async_trait]
impl Tool for OsScrollTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "os_scroll".to_string(),
            description: "Scroll at a grid cell or coordinates. Positive dy scrolls down, negative scrolls up.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "cell": {
                        "type": "string",
                        "description": "Grid cell to scroll at (optional, scrolls at current position if not specified)"
                    },
                    "dx": {
                        "type": "integer",
                        "description": "Horizontal scroll amount (positive = right, negative = left)",
                        "default": 0
                    },
                    "dy": {
                        "type": "integer",
                        "description": "Vertical scroll amount (positive = down, negative = up)",
                        "default": -3
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let dx = params["dx"].as_i64().unwrap_or(0) as i32;
        let dy = params["dy"].as_i64().unwrap_or(-3) as i32;

        let mut desktop = self.desktop.lock().await;

        if let Some(cell) = params["cell"].as_str() {
            match desktop.scroll_at_cell(cell, dx, dy) {
                Ok(()) => Ok(ToolResult::success(format!(
                    "Scrolled at grid cell {} (dx={}, dy={})",
                    cell, dx, dy
                ))),
                Err(e) => Ok(ToolResult::error(format!("Failed to scroll: {}", e))),
            }
        } else {
            // Scroll at current position
            match desktop.input_mut().scroll(dx, dy) {
                Ok(()) => Ok(ToolResult::success(format!(
                    "Scrolled at current position (dx={}, dy={})",
                    dx, dy
                ))),
                Err(e) => Ok(ToolResult::error(format!("Failed to scroll: {}", e))),
            }
        }
    }
}

/// Drag from one location to another
pub struct OsDragTool {
    pub desktop: Arc<Mutex<DesktopManager>>,
}

#[async_trait]
impl Tool for OsDragTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "os_drag".to_string(),
            description: "Drag from one grid cell or coordinates to another. Useful for drag-and-drop, selecting text, etc.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "from_cell": {
                        "type": "string",
                        "description": "Starting grid cell"
                    },
                    "to_cell": {
                        "type": "string",
                        "description": "Ending grid cell"
                    },
                    "from_x": {
                        "type": "integer",
                        "description": "Starting X coordinate (if not using cells)"
                    },
                    "from_y": {
                        "type": "integer",
                        "description": "Starting Y coordinate (if not using cells)"
                    },
                    "to_x": {
                        "type": "integer",
                        "description": "Ending X coordinate (if not using cells)"
                    },
                    "to_y": {
                        "type": "integer",
                        "description": "Ending Y coordinate (if not using cells)"
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let mut desktop = self.desktop.lock().await;

        if let (Some(from_cell), Some(to_cell)) =
            (params["from_cell"].as_str(), params["to_cell"].as_str())
        {
            match desktop.drag_cells(from_cell, to_cell) {
                Ok(()) => Ok(ToolResult::success(format!(
                    "Dragged from {} to {}",
                    from_cell, to_cell
                ))),
                Err(e) => Ok(ToolResult::error(format!("Failed to drag: {}", e))),
            }
        } else if let (Some(from_x), Some(from_y), Some(to_x), Some(to_y)) = (
            params["from_x"].as_i64(),
            params["from_y"].as_i64(),
            params["to_x"].as_i64(),
            params["to_y"].as_i64(),
        ) {
            match desktop.drag(from_x as i32, from_y as i32, to_x as i32, to_y as i32) {
                Ok(()) => Ok(ToolResult::success(format!(
                    "Dragged from ({}, {}) to ({}, {})",
                    from_x, from_y, to_x, to_y
                ))),
                Err(e) => Ok(ToolResult::error(format!("Failed to drag: {}", e))),
            }
        } else {
            Ok(ToolResult::error(
                "Either 'from_cell'+'to_cell' or all coordinate parameters must be provided",
            ))
        }
    }
}

// ============================================================================
// Keyboard Tools
// ============================================================================

/// Type text at current cursor position
pub struct OsTypeTool {
    pub desktop: Arc<Mutex<DesktopManager>>,
}

#[async_trait]
impl Tool for OsTypeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "os_type".to_string(),
            description: "Type text at the current cursor position. Click on an input field first.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The text to type"
                    }
                },
                "required": ["text"]
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let text = params["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'text' parameter"))?;

        let mut desktop = self.desktop.lock().await;

        match desktop.type_text(text) {
            Ok(()) => Ok(ToolResult::success(format!("Typed: '{}'", text))),
            Err(e) => Ok(ToolResult::error(format!("Failed to type text: {}", e))),
        }
    }
}

/// Send keyboard shortcut/hotkey
pub struct OsHotkeyTool {
    pub desktop: Arc<Mutex<DesktopManager>>,
}

#[async_trait]
impl Tool for OsHotkeyTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "os_hotkey".to_string(),
            description: "Send a keyboard shortcut. Examples: 'ctrl+c' (copy), 'ctrl+v' (paste), \
                          'alt+tab' (switch window), 'ctrl+s' (save), 'enter', 'escape', 'tab'.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "keys": {
                        "type": "string",
                        "description": "Key combination like 'ctrl+c', 'alt+tab', 'shift+enter', or single key like 'enter', 'escape'"
                    }
                },
                "required": ["keys"]
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let keys_str = params["keys"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'keys' parameter"))?;

        let mut desktop = self.desktop.lock().await;

        // Parse the key combination
        let parts: Vec<&str> = keys_str.split('+').map(|s| s.trim()).collect();

        if parts.is_empty() {
            return Ok(ToolResult::error("No keys specified"));
        }

        // Last part is the main key, others are modifiers
        let main_key_str = parts.last().unwrap();
        let modifier_strs = &parts[..parts.len().saturating_sub(1)];

        // Parse modifiers
        let mut modifiers = Vec::new();
        for m in modifier_strs {
            match Modifier::from_str(m) {
                Some(modifier) => modifiers.push(modifier),
                None => {
                    return Ok(ToolResult::error(format!(
                        "Unknown modifier: '{}'. Valid: ctrl, alt, shift, meta/win/cmd",
                        m
                    )))
                }
            }
        }

        // Parse main key
        let main_key = match KeyCode::from_str(main_key_str) {
            Some(k) => k,
            None => {
                return Ok(ToolResult::error(format!(
                    "Unknown key: '{}'. Examples: a-z, 0-9, enter, escape, tab, f1-f12, up, down, left, right",
                    main_key_str
                )))
            }
        };

        // Execute
        if modifiers.is_empty() {
            match desktop.key_press(main_key) {
                Ok(()) => Ok(ToolResult::success(format!("Pressed key: {}", main_key_str))),
                Err(e) => Ok(ToolResult::error(format!("Failed to press key: {}", e))),
            }
        } else {
            match desktop.hotkey(&modifiers, main_key) {
                Ok(()) => Ok(ToolResult::success(format!("Sent hotkey: {}", keys_str))),
                Err(e) => Ok(ToolResult::error(format!("Failed to send hotkey: {}", e))),
            }
        }
    }
}

// ============================================================================
// System Tools
// ============================================================================

/// Launch an application
pub struct LaunchAppTool {
    pub desktop: Arc<Mutex<DesktopManager>>,
}

#[async_trait]
impl Tool for LaunchAppTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "launch_app".to_string(),
            description: "Launch an application by name. Works cross-platform.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "app_name": {
                        "type": "string",
                        "description": "Application name (e.g., 'notepad', 'calculator', 'chrome', 'firefox')"
                    }
                },
                "required": ["app_name"]
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let app_name = params["app_name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'app_name' parameter"))?;

        // Platform-specific application launching
        #[cfg(target_os = "windows")]
        {
            let result = std::process::Command::new("cmd")
                .args(["/C", "start", "", app_name])
                .spawn();

            match result {
                Ok(_) => {
                    // Give the app time to start
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    Ok(ToolResult::success(format!("Launched application: {}", app_name)))
                }
                Err(e) => Ok(ToolResult::error(format!("Failed to launch {}: {}", app_name, e))),
            }
        }

        #[cfg(target_os = "macos")]
        {
            let result = std::process::Command::new("open")
                .args(["-a", app_name])
                .spawn();

            match result {
                Ok(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    Ok(ToolResult::success(format!("Launched application: {}", app_name)))
                }
                Err(e) => Ok(ToolResult::error(format!("Failed to launch {}: {}", app_name, e))),
            }
        }

        #[cfg(target_os = "linux")]
        {
            let result = std::process::Command::new(app_name).spawn();

            match result {
                Ok(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    Ok(ToolResult::success(format!("Launched application: {}", app_name)))
                }
                Err(e) => Ok(ToolResult::error(format!("Failed to launch {}: {}", app_name, e))),
            }
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            Ok(ToolResult::error("Application launching not supported on this platform"))
        }
    }
}

/// List open windows
pub struct ListWindowsTool {
    pub desktop: Arc<Mutex<DesktopManager>>,
}

#[async_trait]
impl Tool for ListWindowsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_windows".to_string(),
            description: "List all visible windows on the desktop.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(&self, _params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let desktop = self.desktop.lock().await;

        match desktop.list_windows() {
            Ok(windows) => {
                let window_list: Vec<Value> = windows
                    .iter()
                    .map(|w| {
                        json!({
                            "id": w.id,
                            "title": w.title,
                            "app_name": w.app_name,
                            "position": {"x": w.x, "y": w.y},
                            "size": {"width": w.width, "height": w.height}
                        })
                    })
                    .collect();

                Ok(ToolResult::success_with_data(
                    format!("Found {} windows", windows.len()),
                    json!({ "windows": window_list }),
                ))
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to list windows: {}", e))),
        }
    }
}

/// Wait for a specified duration
pub struct OsWaitTool;

#[async_trait]
impl Tool for OsWaitTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "os_wait".to_string(),
            description: "Wait for a specified number of seconds. Useful for waiting for applications to load.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "seconds": {
                        "type": "number",
                        "description": "Number of seconds to wait (default: 1, max: 30)",
                        "default": 1,
                        "minimum": 0.1,
                        "maximum": 30
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let seconds = params["seconds"].as_f64().unwrap_or(1.0).clamp(0.1, 30.0);
        tokio::time::sleep(tokio::time::Duration::from_secs_f64(seconds)).await;
        Ok(ToolResult::success(format!("Waited for {} seconds", seconds)))
    }
}

// ============================================================================
// Factory function to register OS tools
// ============================================================================

use super::registry::ToolRegistry;

/// Register all OS tools with the given registry
///
/// Note: The desktop manager is shared between all tools via Arc<Mutex<>>
pub fn register_os_tools(registry: &mut ToolRegistry, desktop: Arc<Mutex<DesktopManager>>) {
    registry.register(Arc::new(OsScreenshotTool {
        desktop: desktop.clone(),
    }));
    registry.register(Arc::new(OsClickTool {
        desktop: desktop.clone(),
    }));
    registry.register(Arc::new(OsDoubleClickTool {
        desktop: desktop.clone(),
    }));
    registry.register(Arc::new(OsRightClickTool {
        desktop: desktop.clone(),
    }));
    registry.register(Arc::new(OsMoveTool {
        desktop: desktop.clone(),
    }));
    registry.register(Arc::new(OsScrollTool {
        desktop: desktop.clone(),
    }));
    registry.register(Arc::new(OsDragTool {
        desktop: desktop.clone(),
    }));
    registry.register(Arc::new(OsTypeTool {
        desktop: desktop.clone(),
    }));
    registry.register(Arc::new(OsHotkeyTool {
        desktop: desktop.clone(),
    }));
    registry.register(Arc::new(LaunchAppTool {
        desktop: desktop.clone(),
    }));
    registry.register(Arc::new(ListWindowsTool { desktop }));
    registry.register(Arc::new(OsWaitTool));
}
