use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use super::registry::{Tool, ToolContext, ToolDefinition, ToolResult, ToolRegistry};
use super::memory_tools::{SaveMemoryTool, RecallMemoriesTool};

// =============================================================================
// click_element — Click by index from interactive elements list [PRIMARY]
// =============================================================================

pub struct ClickElementTool;

#[async_trait]
impl Tool for ClickElementTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "click_element".to_string(),
            description: "Click an interactive element by its index from the elements list. This is the PRIMARY way to interact with the desktop. Use the index number shown in [brackets] in the interactive elements list and on the screenshot markers.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "index": {
                        "type": "integer",
                        "description": "The element index from the interactive elements list (e.g., 1, 2, 3)"
                    }
                },
                "required": ["index"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let index = params["index"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'index' parameter"))? as usize;

        ctx.desktop.click_element(index)?;

        Ok(ToolResult::success(format!("Clicked element [{}]", index)))
    }
}

// =============================================================================
// input_text — Click element then type text
// =============================================================================

pub struct InputTextTool;

#[async_trait]
impl Tool for InputTextTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "input_text".to_string(),
            description: "Click a text field by its element index, then type text into it. Use this for filling in forms, search boxes, etc.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "index": {
                        "type": "integer",
                        "description": "The element index of the text field"
                    },
                    "text": {
                        "type": "string",
                        "description": "The text to type"
                    }
                },
                "required": ["index", "text"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let index = params["index"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'index' parameter"))? as usize;
        let text = params["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'text' parameter"))?;

        ctx.desktop.input_text_at_element(index, text)?;

        Ok(ToolResult::success(format!(
            "Clicked element [{}] and typed '{}'",
            index,
            truncate(text, 50)
        )))
    }
}

// =============================================================================
// desktop_click — Click at coordinates [FALLBACK]
// =============================================================================

pub struct DesktopClickTool;

#[async_trait]
impl Tool for DesktopClickTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "desktop_click".to_string(),
            description: "Click at specific coordinates on the screen. ONLY use this as a FALLBACK when the target element is NOT in the interactive elements list (e.g., images, canvas, custom controls). Prefer click_element(index) whenever possible.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "x": {
                        "type": "integer",
                        "description": "X coordinate (in screenshot space)"
                    },
                    "y": {
                        "type": "integer",
                        "description": "Y coordinate (in screenshot space)"
                    },
                    "button": {
                        "type": "string",
                        "enum": ["left", "right", "middle"],
                        "description": "Mouse button (default: left)"
                    },
                    "double_click": {
                        "type": "boolean",
                        "description": "Whether to double-click (default: false)"
                    }
                },
                "required": ["x", "y"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let x = params["x"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'x' parameter"))? as i32;
        let y = params["y"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'y' parameter"))? as i32;
        let button = params["button"].as_str().unwrap_or("left");
        let double_click = params["double_click"].as_bool().unwrap_or(false);

        ctx.desktop.click_at(x, y, button, double_click)?;

        Ok(ToolResult::success(format!(
            "Clicked at ({}, {}) with {} button{}",
            x,
            y,
            button,
            if double_click { " (double)" } else { "" }
        )))
    }
}

// =============================================================================
// desktop_type — Type at current cursor position
// =============================================================================

pub struct DesktopTypeTool;

#[async_trait]
impl Tool for DesktopTypeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "desktop_type".to_string(),
            description: "Type text at the current cursor position without clicking first. Use this when the correct field is already focused.".to_string(),
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

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let text = params["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'text' parameter"))?;

        ctx.desktop.type_text(text)?;

        Ok(ToolResult::success(format!(
            "Typed '{}'",
            truncate(text, 50)
        )))
    }
}

// =============================================================================
// desktop_key — Press key combination
// =============================================================================

pub struct DesktopKeyTool;

#[async_trait]
impl Tool for DesktopKeyTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "desktop_key".to_string(),
            description: "Press a key or key combination. Use '+' to combine modifiers. Examples: 'enter', 'ctrl+c', 'alt+tab', 'ctrl+shift+s', 'win+e', 'f5'.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "Key combination (e.g., 'enter', 'ctrl+c', 'alt+tab', 'ctrl+shift+s')"
                    }
                },
                "required": ["key"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let key = params["key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'key' parameter"))?;

        ctx.desktop.key_press(key)?;

        Ok(ToolResult::success(format!("Pressed key: {}", key)))
    }
}

// =============================================================================
// desktop_scroll — Scroll in a direction
// =============================================================================

pub struct DesktopScrollTool;

#[async_trait]
impl Tool for DesktopScrollTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "desktop_scroll".to_string(),
            description: "Scroll the active window in a direction.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "direction": {
                        "type": "string",
                        "enum": ["up", "down", "left", "right"],
                        "description": "Scroll direction"
                    },
                    "amount": {
                        "type": "integer",
                        "description": "Scroll amount in lines (default: 3)"
                    }
                },
                "required": ["direction"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let direction = params["direction"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'direction' parameter"))?;
        let amount = params["amount"].as_i64().unwrap_or(3) as i32;

        ctx.desktop.scroll(direction, amount)?;

        Ok(ToolResult::success(format!(
            "Scrolled {} by {}",
            direction, amount
        )))
    }
}

// =============================================================================
// desktop_drag — Click and drag
// =============================================================================

pub struct DesktopDragTool;

#[async_trait]
impl Tool for DesktopDragTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "desktop_drag".to_string(),
            description: "Click and drag from one position to another. Coordinates are in screenshot space.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "start_x": { "type": "integer", "description": "Start X coordinate" },
                    "start_y": { "type": "integer", "description": "Start Y coordinate" },
                    "end_x": { "type": "integer", "description": "End X coordinate" },
                    "end_y": { "type": "integer", "description": "End Y coordinate" }
                },
                "required": ["start_x", "start_y", "end_x", "end_y"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let start_x = params["start_x"].as_i64().ok_or_else(|| anyhow::anyhow!("Missing start_x"))? as i32;
        let start_y = params["start_y"].as_i64().ok_or_else(|| anyhow::anyhow!("Missing start_y"))? as i32;
        let end_x = params["end_x"].as_i64().ok_or_else(|| anyhow::anyhow!("Missing end_x"))? as i32;
        let end_y = params["end_y"].as_i64().ok_or_else(|| anyhow::anyhow!("Missing end_y"))? as i32;

        ctx.desktop.drag(start_x, start_y, end_x, end_y)?;

        Ok(ToolResult::success(format!(
            "Dragged from ({}, {}) to ({}, {})",
            start_x, start_y, end_x, end_y
        )))
    }
}

// =============================================================================
// desktop_mouse_move — Move mouse without clicking
// =============================================================================

pub struct DesktopMouseMoveTool;

#[async_trait]
impl Tool for DesktopMouseMoveTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "desktop_mouse_move".to_string(),
            description: "Move the mouse cursor to a position without clicking. Useful for hovering to reveal tooltips or menus.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "x": { "type": "integer", "description": "X coordinate (screenshot space)" },
                    "y": { "type": "integer", "description": "Y coordinate (screenshot space)" }
                },
                "required": ["x", "y"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let x = params["x"].as_i64().ok_or_else(|| anyhow::anyhow!("Missing 'x'"))? as i32;
        let y = params["y"].as_i64().ok_or_else(|| anyhow::anyhow!("Missing 'y'"))? as i32;

        ctx.desktop.mouse_move(x, y)?;

        Ok(ToolResult::success(format!("Moved mouse to ({}, {})", x, y)))
    }
}

// =============================================================================
// desktop_zoom — Capture region at full resolution
// =============================================================================

pub struct DesktopZoomTool;

#[async_trait]
impl Tool for DesktopZoomTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "desktop_zoom".to_string(),
            description: "Capture a region of the screen at full resolution. Use this to read small text that's hard to see in the normal screenshot.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "x": { "type": "integer", "description": "Top-left X (screenshot space)" },
                    "y": { "type": "integer", "description": "Top-left Y (screenshot space)" },
                    "width": { "type": "integer", "description": "Width of the region" },
                    "height": { "type": "integer", "description": "Height of the region" }
                },
                "required": ["x", "y", "width", "height"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let x = params["x"].as_i64().ok_or_else(|| anyhow::anyhow!("Missing 'x'"))? as i32;
        let y = params["y"].as_i64().ok_or_else(|| anyhow::anyhow!("Missing 'y'"))? as i32;
        let width = params["width"].as_i64().ok_or_else(|| anyhow::anyhow!("Missing 'width'"))? as i32;
        let height = params["height"].as_i64().ok_or_else(|| anyhow::anyhow!("Missing 'height'"))? as i32;

        let base64 = ctx.desktop.capture_zoom(x, y, width, height)?;

        Ok(ToolResult::success_with_data(
            format!("Captured zoom region {}x{} at ({}, {})", width, height, x, y),
            json!({ "screenshot_base64": base64 }),
        ))
    }
}

// =============================================================================
// open_application — Launch an application
// =============================================================================

pub struct OpenApplicationTool;

#[async_trait]
impl Tool for OpenApplicationTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "open_application".to_string(),
            description: "Launch an application by name or path. Examples: 'notepad', 'chrome', 'cmd', 'explorer', 'calc'. On Windows, you can also use full paths.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Application name or path (e.g., 'notepad', 'chrome', 'cmd')"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let path = params["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' parameter"))?;

        // Try to launch the application
        #[cfg(target_os = "windows")]
        {
            // On Windows, use 'start' for common apps or direct path
            let result = std::process::Command::new("cmd")
                .args(["/C", "start", "", path])
                .spawn();

            match result {
                Ok(_) => {
                    // Give the app time to open
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                    Ok(ToolResult::success(format!("Launched '{}'", path)))
                }
                Err(e) => Ok(ToolResult::error(format!("Failed to launch '{}': {}", path, e))),
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let result = std::process::Command::new(path).spawn();
            match result {
                Ok(_) => {
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                    Ok(ToolResult::success(format!("Launched '{}'", path)))
                }
                Err(e) => {
                    // Try with 'open' on macOS or 'xdg-open' on Linux
                    #[cfg(target_os = "macos")]
                    let fallback = std::process::Command::new("open").arg("-a").arg(path).spawn();
                    #[cfg(target_os = "linux")]
                    let fallback = std::process::Command::new("xdg-open").arg(path).spawn();

                    match fallback {
                        Ok(_) => {
                            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                            Ok(ToolResult::success(format!("Launched '{}'", path)))
                        }
                        Err(_) => Ok(ToolResult::error(format!("Failed to launch '{}': {}", path, e))),
                    }
                }
            }
        }
    }
}

// =============================================================================
// list_windows — List all visible windows
// =============================================================================

pub struct ListWindowsTool;

#[async_trait]
impl Tool for ListWindowsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_windows".to_string(),
            description: "List all visible windows with their titles, positions, and sizes.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(&self, _params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let windows = ctx.desktop.list_windows()?;

        if windows.is_empty() {
            return Ok(ToolResult::success("No visible windows found"));
        }

        let list: Vec<String> = windows
            .iter()
            .enumerate()
            .map(|(i, w)| {
                format!(
                    "{}. \"{}\" at ({}, {}) size {}x{}",
                    i + 1,
                    w.title,
                    w.x,
                    w.y,
                    w.width,
                    w.height
                )
            })
            .collect();

        Ok(ToolResult::success_with_data(
            format!("Found {} windows:\n{}", windows.len(), list.join("\n")),
            json!({ "windows": windows }),
        ))
    }
}

// =============================================================================
// focus_window — Bring window to foreground
// =============================================================================

pub struct FocusWindowTool;

#[async_trait]
impl Tool for FocusWindowTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "focus_window".to_string(),
            description: "Bring a window to the foreground by its title (substring match). Use list_windows first to see available windows.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Window title to search for (substring match)"
                    },
                    "process_name": {
                        "type": "string",
                        "description": "Process name to search for (alternative to title)"
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let title = params["title"].as_str();
        let process_name = params["process_name"].as_str();

        if title.is_none() && process_name.is_none() {
            return Ok(ToolResult::error("Provide either 'title' or 'process_name'"));
        }

        ctx.desktop.focus_window(title, process_name)?;

        let search = title.or(process_name).unwrap_or("unknown");
        // Give time for the window to come to front
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        Ok(ToolResult::success(format!("Focused window matching '{}'", search)))
    }
}

// =============================================================================
// wait — Pause between actions
// =============================================================================

pub struct WaitTool;

#[async_trait]
impl Tool for WaitTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "wait".to_string(),
            description: "Wait for a specified number of seconds before the next action. Use when you need to wait for an application to load or an animation to complete.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "seconds": {
                        "type": "integer",
                        "description": "Number of seconds to wait (1-30)"
                    }
                },
                "required": ["seconds"]
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let seconds = params["seconds"]
            .as_i64()
            .unwrap_or(3)
            .clamp(1, 30) as u64;

        tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;

        Ok(ToolResult::success(format!("Waited {} seconds", seconds)))
    }
}

// =============================================================================
// done — Signal task completion
// =============================================================================

pub struct DoneTool;

#[async_trait]
impl Tool for DoneTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "done".to_string(),
            description: "Signal that the task is complete. Provide a summary of what was accomplished.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "Summary of what was accomplished (markdown supported)"
                    },
                    "success": {
                        "type": "boolean",
                        "description": "Whether the task was completed successfully (default: true)"
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
        let success = params["success"].as_bool().unwrap_or(true);

        Ok(ToolResult::done(text, success))
    }
}

// =============================================================================
// Register all desktop tools
// =============================================================================

pub fn register_all_tools(registry: &mut ToolRegistry) {
    // Primary interaction
    registry.register(Arc::new(ClickElementTool));
    registry.register(Arc::new(InputTextTool));

    // Coordinate fallback
    registry.register(Arc::new(DesktopClickTool));
    registry.register(Arc::new(DesktopTypeTool));

    // Keyboard
    registry.register(Arc::new(DesktopKeyTool));

    // Navigation
    registry.register(Arc::new(DesktopScrollTool));
    registry.register(Arc::new(DesktopDragTool));
    registry.register(Arc::new(DesktopMouseMoveTool));

    // Inspection
    registry.register(Arc::new(DesktopZoomTool));

    // Application management
    registry.register(Arc::new(OpenApplicationTool));
    registry.register(Arc::new(ListWindowsTool));
    registry.register(Arc::new(FocusWindowTool));

    // Timing
    registry.register(Arc::new(WaitTool));

    // Memory
    registry.register(Arc::new(SaveMemoryTool));
    registry.register(Arc::new(RecallMemoriesTool));

    // Completion
    registry.register(Arc::new(DoneTool));
}

// =============================================================================
// Helper
// =============================================================================

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}
