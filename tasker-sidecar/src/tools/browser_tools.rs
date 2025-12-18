use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::registry::{Tool, ToolContext, ToolDefinition, ToolResult};
use crate::runs::RunFile;

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse an integer parameter robustly - handles both integer and string values
fn parse_int_param(params: &Value, name: &str) -> Option<i32> {
    // Try as integer first
    if let Some(i) = params[name].as_i64() {
        return Some(i as i32);
    }
    // Try as string and parse
    if let Some(s) = params[name].as_str() {
        if let Ok(i) = s.parse::<i32>() {
            return Some(i);
        }
    }
    // Try common alternative names for index
    if name == "index" {
        // Sometimes LLMs use "element" or "element_index" instead
        for alt in ["element", "element_index", "idx", "id"] {
            if let Some(i) = params[alt].as_i64() {
                return Some(i as i32);
            }
            if let Some(s) = params[alt].as_str() {
                if let Ok(i) = s.parse::<i32>() {
                    return Some(i);
                }
            }
        }
    }
    None
}

// ============================================================================
// Navigation Tools
// ============================================================================

/// Search Google and navigate to results
pub struct SearchTool;

#[async_trait]
impl Tool for SearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_google".to_string(),
            description: "Search Google for a query and navigate to the search results page".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query to look up on Google"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let query = params["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'query' parameter"))?;

        let search_url = format!(
            "https://www.google.com/search?q={}",
            urlencoding::encode(query)
        );

        ctx.browser.navigate(&search_url).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        Ok(ToolResult::success(format!(
            "Searched Google for: '{}'. Navigate to specific results as needed.",
            query
        )))
    }
}

/// Navigate to a specific URL
pub struct NavigateTool;

#[async_trait]
impl Tool for NavigateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "go_to_url".to_string(),
            description: "Navigate to a specific URL in the browser".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to navigate to"
                    }
                },
                "required": ["url"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let url = params["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'url' parameter"))?;

        ctx.browser.navigate(url).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let current_url = ctx.browser.current_url().await.unwrap_or_default();
        Ok(ToolResult::success(format!("Navigated to: {}", current_url)))
    }
}

/// Go back in browser history
pub struct GoBackTool;

#[async_trait]
impl Tool for GoBackTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "go_back".to_string(),
            description: "Go back to the previous page in browser history".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(&self, _params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        ctx.browser.evaluate("window.history.back(); true").await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let current_url = ctx.browser.current_url().await.unwrap_or_default();
        Ok(ToolResult::success(format!("Went back. Now at: {}", current_url)))
    }
}

/// Wait for specified duration
pub struct WaitTool;

#[async_trait]
impl Tool for WaitTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "wait".to_string(),
            description: "Wait for a specified number of seconds before continuing".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "seconds": {
                        "type": "integer",
                        "description": "Number of seconds to wait (default: 3)",
                        "default": 3,
                        "minimum": 1
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let seconds = params["seconds"].as_i64().unwrap_or(3).max(1);
        tokio::time::sleep(tokio::time::Duration::from_secs(seconds as u64)).await;
        Ok(ToolResult::success(format!("Waited for {} seconds", seconds)))
    }
}

// ============================================================================
// Interaction Tools
// ============================================================================

/// Click on an element
pub struct ClickTool;

#[async_trait]
impl Tool for ClickTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "click_element".to_string(),
            description: "Click on a page element identified by index from the interactive elements list".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "index": {
                        "type": "integer",
                        "description": "The 1-based index of the element to click (e.g., [1], [2], [3])"
                    }
                },
                "required": ["index"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let index = match parse_int_param(&params, "index") {
            Some(i) => i,
            None => return Ok(ToolResult::error(
                "Missing 'index' parameter. Use the element number from the list, e.g. index: 5"
            )),
        };

        // Look up backend_node_id from selector map
        let selector_map = ctx.selector_map.read().await;
        let backend_id = match selector_map.get_backend_id(index) {
            Some(id) => id,
            None => {
                let msg = if selector_map.is_empty() {
                    format!("Element index {} not found. No interactive elements on page.", index)
                } else {
                    format!("Element index {} not found. Valid indices: 1-{}", index, selector_map.len())
                };
                return Ok(ToolResult::error(msg));
            }
        };
        drop(selector_map); // Release read lock before async operations

        match ctx.browser.click_by_backend_id(backend_id).await {
            Ok(()) => Ok(ToolResult::success(format!("Clicked element [{}]", index))),
            Err(e) => Ok(ToolResult::error(format!("Failed to click element [{}]: {}", index, e)))
        }
    }
}

/// Hover over an element to reveal tooltips, dropdowns, or hidden content
pub struct HoverTool;

#[async_trait]
impl Tool for HoverTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "hover_element".to_string(),
            description: "Hover over an element to trigger hover states, reveal tooltips, dropdown menus, or hidden content".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "index": {
                        "type": "integer",
                        "description": "The 1-based index of the element to hover over (e.g., [1], [2], [3])"
                    }
                },
                "required": ["index"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let index = match parse_int_param(&params, "index") {
            Some(i) => i,
            None => return Ok(ToolResult::error(
                "Missing 'index' parameter. Use the element number from the list, e.g. index: 5"
            )),
        };

        // Look up backend_node_id from selector map
        let selector_map = ctx.selector_map.read().await;
        let backend_id = match selector_map.get_backend_id(index) {
            Some(id) => id,
            None => {
                let msg = if selector_map.is_empty() {
                    format!("Element index {} not found. No interactive elements on page.", index)
                } else {
                    format!("Element index {} not found. Valid indices: 1-{}", index, selector_map.len())
                };
                return Ok(ToolResult::error(msg));
            }
        };
        drop(selector_map); // Release read lock before async operations

        match ctx.browser.hover_by_backend_id(backend_id).await {
            Ok(()) => Ok(ToolResult::success(format!("Hovered over element [{}]. Check the page for tooltips or revealed content.", index))),
            Err(e) => Ok(ToolResult::error(format!("Failed to hover over element [{}]: {}", index, e)))
        }
    }
}

/// Input text into a field
pub struct InputTextTool;

#[async_trait]
impl Tool for InputTextTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "input_text".to_string(),
            description: "Type text into an input field identified by index. NOTE: This appends to existing text. Use clear_input first if the field has text you want to replace.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "index": {
                        "type": "integer",
                        "description": "The 1-based index of the input element (e.g., [1], [2], [3])"
                    },
                    "text": {
                        "type": "string",
                        "description": "The text to type into the field"
                    }
                },
                "required": ["index", "text"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let index = match parse_int_param(&params, "index") {
            Some(i) => i,
            None => return Ok(ToolResult::error(
                "Missing 'index' parameter. Use the element number from the list, e.g. index: 3"
            )),
        };
        let text = params["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'text' parameter"))?;

        // Look up backend_node_id from selector map
        let selector_map = ctx.selector_map.read().await;
        let backend_id = match selector_map.get_backend_id(index) {
            Some(id) => id,
            None => {
                let msg = if selector_map.is_empty() {
                    format!("Element index {} not found. No interactive elements on page.", index)
                } else {
                    format!("Element index {} not found. Valid indices: 1-{}", index, selector_map.len())
                };
                return Ok(ToolResult::error(msg));
            }
        };
        drop(selector_map);

        match ctx.browser.type_by_backend_id(backend_id, text).await {
            Ok(()) => Ok(ToolResult::success(format!(
                "Typed '{}' into element [{}]",
                text, index
            ))),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to type into element [{}]: {}",
                index, e
            )))
        }
    }
}

/// Clear an input field
pub struct ClearInputTool;

#[async_trait]
impl Tool for ClearInputTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "clear_input".to_string(),
            description: "Clear an input field by selecting all text and deleting it. Use this before input_text if the field already has text you want to replace.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "index": {
                        "type": "integer",
                        "description": "The 1-based index of the input element (e.g., [1], [2], [3])"
                    }
                },
                "required": ["index"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let index = match parse_int_param(&params, "index") {
            Some(i) => i,
            None => return Ok(ToolResult::error(
                "Missing 'index' parameter. Use the element number from the list, e.g. index: 3"
            )),
        };

        // Look up backend_node_id from selector map
        let selector_map = ctx.selector_map.read().await;
        let backend_id = match selector_map.get_backend_id(index) {
            Some(id) => id,
            None => {
                let msg = if selector_map.is_empty() {
                    format!("Element index {} not found. No interactive elements on page.", index)
                } else {
                    format!("Element index {} not found. Valid indices: 1-{}", index, selector_map.len())
                };
                return Ok(ToolResult::error(msg));
            }
        };
        drop(selector_map);

        match ctx.browser.clear_input_by_backend_id(backend_id).await {
            Ok(()) => Ok(ToolResult::success(format!("Cleared input field [{}]", index))),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to clear input field [{}]: {}",
                index, e
            )))
        }
    }
}

/// Scroll the page
pub struct ScrollTool;

#[async_trait]
impl Tool for ScrollTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "scroll_down".to_string(),
            description: "Scroll down the page by a specified amount or to an element".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "amount": {
                        "type": "integer",
                        "description": "Pixels to scroll down. Default is 500.",
                        "default": 500
                    },
                    "to_element": {
                        "type": "integer",
                        "description": "1-based index of element to scroll into view (alternative to amount)"
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        if let Some(index) = parse_int_param(&params, "to_element") {
            // Look up element from selector map
            let selector_map = ctx.selector_map.read().await;
            let element = match selector_map.get_element_by_index(index) {
                Some(el) => el.clone(),
                None => {
                    let msg = if selector_map.is_empty() {
                        format!("Element index {} not found. No interactive elements on page.", index)
                    } else {
                        format!("Element index {} not found. Valid indices: 1-{}", index, selector_map.len())
                    };
                    return Ok(ToolResult::error(msg));
                }
            };
            drop(selector_map);

            // Scroll to element's position
            let scroll_y = element.bounds.y as i32;
            ctx.browser.scroll(0, scroll_y).await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            return Ok(ToolResult::success(format!("Scrolled to element [{}]", index)));
        }

        let amount = params["amount"].as_i64().unwrap_or(500);
        ctx.browser.scroll(0, amount as i32).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        Ok(ToolResult::success(format!("Scrolled down {} pixels", amount)))
    }
}

/// Scroll up the page
pub struct ScrollUpTool;

#[async_trait]
impl Tool for ScrollUpTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "scroll_up".to_string(),
            description: "Scroll up the page by a specified amount".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "amount": {
                        "type": "integer",
                        "description": "Pixels to scroll up. Default is one viewport height.",
                        "default": null
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let amount = params["amount"].as_i64().unwrap_or(500);
        ctx.browser.scroll(0, -(amount as i32)).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        Ok(ToolResult::success(format!("Scrolled up by {} pixels", amount)))
    }
}

/// Send keyboard keys
pub struct SendKeysTool;

#[async_trait]
impl Tool for SendKeysTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "send_keys".to_string(),
            description: "Send keyboard keys like Enter, Tab, Escape, arrow keys, etc.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "keys": {
                        "type": "string",
                        "description": "Key to send: 'Enter', 'Tab', 'Escape', 'Backspace', 'Delete', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Space', 'Home', 'End', 'PageUp', 'PageDown'"
                    }
                },
                "required": ["keys"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let keys = params["keys"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'keys' parameter"))?;

        // Use CDP-based key press for reliable keyboard input
        match ctx.browser.press_key(keys).await {
            Ok(()) => Ok(ToolResult::success(format!("Pressed key: {}", keys))),
            Err(e) => Ok(ToolResult::error(format!("Failed to press key '{}': {}", keys, e)))
        }
    }
}

// ============================================================================
// Extraction Tools
// ============================================================================

/// Extract content from page
pub struct ExtractTool;

#[async_trait]
impl Tool for ExtractTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "extract_content".to_string(),
            description: "Extract and return specific content from the page based on a goal".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "goal": {
                        "type": "string",
                        "description": "What information to extract (e.g., 'product prices', 'article text', 'all links')"
                    }
                },
                "required": ["goal"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let goal = params["goal"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'goal' parameter"))?;

        // Get page content for extraction
        let html = ctx.browser.get_dom().await?;

        // For now, return the page content - the LLM can parse it
        // In production, this could use more sophisticated extraction
        let content_preview = if html.len() > 5000 {
            format!("{}...\n[Content truncated. Total {} chars]", &html[..5000], html.len())
        } else {
            html
        };

        Ok(ToolResult::success_with_data(
            format!("Extracted content for goal: '{}'. Review the data field for details.", goal),
            json!({ "goal": goal, "content": content_preview })
        ))
    }
}

/// Take a screenshot
pub struct ScreenshotTool;

#[async_trait]
impl Tool for ScreenshotTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "screenshot".to_string(),
            description: "Take a screenshot of the current page state".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(&self, _params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let screenshot_base64 = ctx.browser.screenshot().await?;

        Ok(ToolResult::success_with_data(
            "Screenshot captured successfully".to_string(),
            json!({ "screenshot": screenshot_base64 })
        ))
    }
}

/// Find text on page
pub struct FindTextTool;

#[async_trait]
impl Tool for FindTextTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "find_text".to_string(),
            description: "Search for specific text on the page and return its location".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The text to search for on the page"
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

        let script = format!(
            r#"(function() {{
            const searchText = '{}';
            const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
            const matches = [];
            let node;
            while (node = walker.nextNode()) {{
                if (node.textContent.toLowerCase().includes(searchText.toLowerCase())) {{
                    const range = document.createRange();
                    range.selectNodeContents(node);
                    const rect = range.getBoundingClientRect();
                    matches.push({{
                        text: node.textContent.trim().substring(0, 100),
                        x: Math.round(rect.x),
                        y: Math.round(rect.y)
                    }});
                }}
            }}
            return JSON.stringify(matches.slice(0, 10));
            }})()"#,
            text.replace('\'', "\\'")
        );

        let result = ctx.browser.evaluate(&script).await?;
        let matches: Vec<Value> = serde_json::from_str(result.as_str().unwrap_or("[]")).unwrap_or_default();

        if matches.is_empty() {
            Ok(ToolResult::success(format!("Text '{}' not found on page", text)))
        } else {
            Ok(ToolResult::success_with_data(
                format!("Found {} occurrences of '{}'", matches.len(), text),
                json!({ "matches": matches })
            ))
        }
    }
}

// ============================================================================
// Form Tools
// ============================================================================

/// Select dropdown option
pub struct SelectDropdownTool;

#[async_trait]
impl Tool for SelectDropdownTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "select_dropdown_option".to_string(),
            description: "Select an option from a dropdown menu by index and option value/text".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "index": {
                        "type": "integer",
                        "description": "The 1-based index of the dropdown element (e.g., [1], [2], [3])"
                    },
                    "option": {
                        "type": "string",
                        "description": "The value or visible text of the option to select"
                    }
                },
                "required": ["index", "option"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let index = match parse_int_param(&params, "index") {
            Some(i) => i,
            None => return Ok(ToolResult::error(
                "Missing 'index' parameter. Use the dropdown element number from the list."
            )),
        };
        let option = params["option"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'option' parameter"))?;

        // Look up backend_node_id from selector map
        let selector_map = ctx.selector_map.read().await;
        let element = match selector_map.get_element_by_index(index) {
            Some(el) => el.clone(),
            None => {
                let msg = if selector_map.is_empty() {
                    format!("Element index {} not found. No interactive elements on page.", index)
                } else {
                    format!("Element index {} not found. Valid indices: 1-{}", index, selector_map.len())
                };
                return Ok(ToolResult::error(msg));
            }
        };
        let backend_id = element.backend_node_id;
        drop(selector_map);

        // Check if element is a select
        if element.tag.to_lowercase() != "select" {
            return Ok(ToolResult::error(format!(
                "Element [{}] is not a select/dropdown (found {})",
                index, element.tag
            )));
        }

        // Use backend_node_id for reliable selection (not coordinate-based)
        match ctx.browser.select_option_by_backend_id(backend_id, option).await {
            Ok(()) => Ok(ToolResult::success(format!(
                "Selected option '{}' in dropdown [{}]",
                option, index
            ))),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to select option '{}' in dropdown [{}]: {}",
                option, index, e
            )))
        }
    }
}

/// Get dropdown options
pub struct GetDropdownOptionsTool;

#[async_trait]
impl Tool for GetDropdownOptionsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_dropdown_options".to_string(),
            description: "Get all available options from a dropdown/select element".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "index": {
                        "type": "integer",
                        "description": "The 1-based index of the dropdown element"
                    }
                },
                "required": ["index"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let index = match parse_int_param(&params, "index") {
            Some(i) => i,
            None => return Ok(ToolResult::error(
                "Missing 'index' parameter. Use the dropdown element number from the list."
            )),
        };

        // Look up element from selector map
        let selector_map = ctx.selector_map.read().await;
        let element = match selector_map.get_element_by_index(index) {
            Some(el) => el.clone(),
            None => {
                let msg = if selector_map.is_empty() {
                    format!("Element index {} not found. No interactive elements on page.", index)
                } else {
                    format!("Element index {} not found. Valid indices: 1-{}", index, selector_map.len())
                };
                return Ok(ToolResult::error(msg));
            }
        };
        drop(selector_map);

        // Check if element is a select
        if element.tag.to_lowercase() != "select" {
            return Ok(ToolResult::error(format!(
                "Element [{}] is not a select/dropdown (found {})",
                index, element.tag
            )));
        }

        // Use cached options from SimplifiedElement (extracted during DOM parsing)
        match &element.select_options {
            Some(options) if !options.is_empty() => {
                let options_json: Vec<Value> = options.iter().map(|o| json!({
                    "value": o.value,
                    "text": o.text,
                    "selected": o.selected
                })).collect();

                Ok(ToolResult::success_with_data(
                    format!("Found {} options in dropdown [{}]", options.len(), index),
                    json!({ "options": options_json })
                ))
            }
            _ => Ok(ToolResult::error(format!(
                "No options found for dropdown [{}]",
                index
            )))
        }
    }
}

/// Upload a file
pub struct UploadFileTool;

#[async_trait]
impl Tool for UploadFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "upload_file".to_string(),
            description: "Upload a file to a file input element".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "index": {
                        "type": "integer",
                        "description": "The index of the file input element"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "The local path to the file to upload"
                    }
                },
                "required": ["index", "file_path"]
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let index = match parse_int_param(&params, "index") {
            Some(i) => i,
            None => return Ok(ToolResult::error(
                "Missing 'index' parameter. Use the file input element number from the list."
            )),
        };
        let file_path = params["file_path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'file_path' parameter"))?;

        // Check if file exists
        if !std::path::Path::new(file_path).exists() {
            return Ok(ToolResult::error(format!("File not found: {}", file_path)));
        }

        // File upload requires CDP FileChooser interaction - placeholder for now
        Ok(ToolResult::error(format!(
            "File upload to element {} with path '{}' - feature requires CDP FileChooser implementation",
            index, file_path
        )))
    }
}

// ============================================================================
// Tab Management Tools
// ============================================================================

/// Open a new tab and navigate to URL
pub struct NewTabTool;

#[async_trait]
impl Tool for NewTabTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "new_tab".to_string(),
            description: "Open a new browser tab and navigate to a URL. The new tab becomes the active tab.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to open in the new tab"
                    }
                },
                "required": ["url"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let url = params["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'url' parameter"))?;

        match ctx.browser.new_tab(url).await {
            Ok(tab_index) => Ok(ToolResult::success(format!(
                "Opened new tab {} at: {}",
                tab_index, url
            ))),
            Err(e) => Ok(ToolResult::error(format!("Failed to open new tab: {}", e)))
        }
    }
}

/// Switch to a different tab
pub struct SwitchTabTool;

#[async_trait]
impl Tool for SwitchTabTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "switch_tab".to_string(),
            description: "Switch to a different browser tab by index (0-based). Use list_tabs to see available tabs.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "index": {
                        "type": "integer",
                        "description": "The index of the tab to switch to (0-based)"
                    }
                },
                "required": ["index"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let index = match parse_int_param(&params, "index") {
            Some(i) => i as usize,
            None => return Ok(ToolResult::error(
                "Missing 'index' parameter. Use the tab index (0-based)."
            )),
        };

        match ctx.browser.switch_tab(index).await {
            Ok(()) => {
                let url = ctx.browser.current_url().await.unwrap_or_default();
                Ok(ToolResult::success(format!(
                    "Switched to tab {}. URL: {}",
                    index, url
                )))
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to switch tab: {}", e)))
        }
    }
}

/// Close a tab by index
pub struct CloseTabTool;

#[async_trait]
impl Tool for CloseTabTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "close_tab".to_string(),
            description: "Close a browser tab by index (0-based). Cannot close the last remaining tab.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "index": {
                        "type": "integer",
                        "description": "The index of the tab to close (0-based)"
                    }
                },
                "required": ["index"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let index = match parse_int_param(&params, "index") {
            Some(i) => i as usize,
            None => return Ok(ToolResult::error(
                "Missing 'index' parameter. Use the tab index (0-based)."
            )),
        };

        match ctx.browser.close_tab(index).await {
            Ok(()) => {
                let active = ctx.browser.active_tab_index().await;
                Ok(ToolResult::success(format!(
                    "Closed tab {}. Active tab is now {}",
                    index, active
                )))
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to close tab: {}", e)))
        }
    }
}

/// List all open tabs
pub struct ListTabsTool;

#[async_trait]
impl Tool for ListTabsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_tabs".to_string(),
            description: "List all open browser tabs with their indices and URLs".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(&self, _params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        match ctx.browser.list_tabs().await {
            Ok(tabs) => {
                let active = ctx.browser.active_tab_index().await;
                let tab_list: Vec<Value> = tabs.iter().map(|(i, url)| {
                    json!({
                        "index": i,
                        "url": url,
                        "active": *i == active
                    })
                }).collect();

                Ok(ToolResult::success_with_data(
                    format!("{} tabs open (active: {})", tabs.len(), active),
                    json!({ "tabs": tab_list })
                ))
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to list tabs: {}", e)))
        }
    }
}

// ============================================================================
// JavaScript Tool
// ============================================================================

/// Execute custom JavaScript
pub struct EvaluateJsTool;

#[async_trait]
impl Tool for EvaluateJsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "execute_javascript".to_string(),
            description: "Execute custom JavaScript code in the browser context".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "script": {
                        "type": "string",
                        "description": "The JavaScript code to execute"
                    }
                },
                "required": ["script"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let script = params["script"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'script' parameter"))?;

        match ctx.browser.evaluate(script).await {
            Ok(result) => Ok(ToolResult::success_with_data(
                "JavaScript executed successfully".to_string(),
                json!({ "result": result })
            )),
            Err(e) => Ok(ToolResult::error(format!("JavaScript error: {}", e)))
        }
    }
}

// ============================================================================
// File Tools (Database-backed storage for run files)
// ============================================================================

/// List files stored for the current run
pub struct ListFilesTool;

#[async_trait]
impl Tool for ListFilesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_files".to_string(),
            description: "List all files stored for the current run".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(&self, _params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let repo = match &ctx.file_repository {
            Some(r) => r,
            None => return Ok(ToolResult::error("File storage not available")),
        };

        match repo.list_files_for_run(&ctx.run_id) {
            Ok(files) => {
                if files.is_empty() {
                    return Ok(ToolResult::success("No files stored for this run"));
                }

                let file_list: Vec<Value> = files
                    .iter()
                    .map(|f| {
                        json!({
                            "file_path": f.file_path,
                            "file_name": f.file_name,
                            "mime_type": f.mime_type,
                            "size_bytes": f.file_size,
                            "created_at": f.created_at.to_rfc3339()
                        })
                    })
                    .collect();

                Ok(ToolResult::success_with_data(
                    format!("Found {} files", files.len()),
                    json!({ "files": file_list }),
                ))
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to list files: {}", e))),
        }
    }
}

/// Read a file from storage
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file from storage. Use virtual paths like '/output/data.csv'.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The virtual path to the file (e.g., '/output/report.csv', '/data/results.json')"
                    }
                },
                "required": ["file_path"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let file_path = params["file_path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'file_path' parameter"))?;

        let repo = match &ctx.file_repository {
            Some(r) => r,
            None => return Ok(ToolResult::error("File storage not available")),
        };

        match repo.get_file_by_path(&ctx.run_id, file_path) {
            Ok(Some(file)) => {
                // Try to convert to string for text files
                match String::from_utf8(file.content.clone()) {
                    Ok(text) => {
                        let preview = if text.len() > 10000 {
                            format!(
                                "{}...\n[Truncated. Total {} chars]",
                                &text[..10000],
                                text.len()
                            )
                        } else {
                            text
                        };
                        Ok(ToolResult::success_with_data(
                            format!("Read file: {}", file_path),
                            json!({ "content": preview, "mime_type": file.mime_type }),
                        ))
                    }
                    Err(_) => {
                        // Binary file - return base64
                        use base64::Engine;
                        Ok(ToolResult::success_with_data(
                            format!(
                                "Read binary file: {} ({} bytes)",
                                file_path, file.file_size
                            ),
                            json!({
                                "content_base64": base64::engine::general_purpose::STANDARD.encode(&file.content),
                                "mime_type": file.mime_type,
                                "is_binary": true
                            }),
                        ))
                    }
                }
            }
            Ok(None) => Ok(ToolResult::error(format!("File not found: {}", file_path))),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to read file '{}': {}",
                file_path, e
            ))),
        }
    }
}

/// Write content to a file in storage
pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write_file".to_string(),
            description: "Write content to a file in storage (creates or overwrites). Use virtual paths like '/output/data.csv'. Maximum file size is 50 MB.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The virtual path to write to (e.g., '/output/report.csv', '/data/results.json')"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    }
                },
                "required": ["file_path", "content"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let file_path = params["file_path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'file_path' parameter"))?;
        let content = params["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'content' parameter"))?;

        let repo = match &ctx.file_repository {
            Some(r) => r,
            None => return Ok(ToolResult::error("File storage not available")),
        };

        let file = RunFile::new(
            ctx.run_id.clone(),
            ctx.workflow_id.clone(),
            file_path.to_string(),
            content.as_bytes().to_vec(),
        );

        match repo.upsert_file(&file) {
            Ok(()) => Ok(ToolResult::success(format!(
                "Wrote {} bytes to '{}'",
                content.len(),
                file_path
            ))),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to write file '{}': {}",
                file_path, e
            ))),
        }
    }
}

/// Replace content in a file
pub struct ReplaceInFileTool;

#[async_trait]
impl Tool for ReplaceInFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "replace_in_file".to_string(),
            description: "Find and replace text in a file stored in the database".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The virtual path to the file"
                    },
                    "find": {
                        "type": "string",
                        "description": "The text to find"
                    },
                    "replace": {
                        "type": "string",
                        "description": "The text to replace with"
                    }
                },
                "required": ["file_path", "find", "replace"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let file_path = params["file_path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'file_path' parameter"))?;
        let find = params["find"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'find' parameter"))?;
        let replace = params["replace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'replace' parameter"))?;

        let repo = match &ctx.file_repository {
            Some(r) => r,
            None => return Ok(ToolResult::error("File storage not available")),
        };

        // Read existing file
        let file = match repo.get_file_by_path(&ctx.run_id, file_path) {
            Ok(Some(f)) => f,
            Ok(None) => return Ok(ToolResult::error(format!("File not found: {}", file_path))),
            Err(e) => return Ok(ToolResult::error(format!("Failed to read file: {}", e))),
        };

        // Convert to string
        let content = match String::from_utf8(file.content) {
            Ok(s) => s,
            Err(_) => return Ok(ToolResult::error("Cannot replace in binary file")),
        };

        let count = content.matches(find).count();
        if count == 0 {
            return Ok(ToolResult::error(format!(
                "Text '{}' not found in file",
                find
            )));
        }

        let new_content = content.replace(find, replace);

        // Create updated file
        let updated_file = RunFile::new(
            ctx.run_id.clone(),
            ctx.workflow_id.clone(),
            file_path.to_string(),
            new_content.as_bytes().to_vec(),
        );

        match repo.upsert_file(&updated_file) {
            Ok(()) => Ok(ToolResult::success(format!(
                "Replaced {} occurrences in '{}'",
                count, file_path
            ))),
            Err(e) => Ok(ToolResult::error(format!("Failed to write file: {}", e))),
        }
    }
}

// ============================================================================
// Completion Tool
// ============================================================================

/// Mark task as done
pub struct DoneTool;

#[async_trait]
impl Tool for DoneTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "done".to_string(),
            description: "Signal that the task is complete and provide a summary of what was accomplished".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "A summary of what was accomplished"
                    },
                    "success": {
                        "type": "boolean",
                        "description": "Whether the task was completed successfully",
                        "default": true
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

        Ok(ToolResult::done(text.to_string(), success))
    }
}

// ============================================================================
// Factory function to register all tools
// ============================================================================

use super::registry::ToolRegistry;

pub fn register_all_tools(registry: &mut ToolRegistry) {
    use std::sync::Arc;

    // Navigation
    registry.register(Arc::new(SearchTool));
    registry.register(Arc::new(NavigateTool));
    registry.register(Arc::new(GoBackTool));
    registry.register(Arc::new(WaitTool));

    // Interaction
    registry.register(Arc::new(ClickTool));
    registry.register(Arc::new(HoverTool));
    registry.register(Arc::new(InputTextTool));
    registry.register(Arc::new(ClearInputTool));
    registry.register(Arc::new(ScrollTool));
    registry.register(Arc::new(ScrollUpTool));
    registry.register(Arc::new(SendKeysTool));

    // Extraction
    registry.register(Arc::new(ExtractTool));
    registry.register(Arc::new(ScreenshotTool));
    registry.register(Arc::new(FindTextTool));

    // Forms
    registry.register(Arc::new(SelectDropdownTool));
    registry.register(Arc::new(GetDropdownOptionsTool));
    registry.register(Arc::new(UploadFileTool));

    // Tabs
    registry.register(Arc::new(NewTabTool));
    registry.register(Arc::new(SwitchTabTool));
    registry.register(Arc::new(CloseTabTool));
    registry.register(Arc::new(ListTabsTool));

    // JavaScript
    registry.register(Arc::new(EvaluateJsTool));

    // Files
    registry.register(Arc::new(ListFilesTool));
    registry.register(Arc::new(ReadFileTool));
    registry.register(Arc::new(WriteFileTool));
    registry.register(Arc::new(ReplaceInFileTool));

    // Memory
    use super::memory_tools::{DeleteMemoryTool, RecallMemoriesTool, SaveMemoryTool};
    registry.register(Arc::new(SaveMemoryTool));
    registry.register(Arc::new(RecallMemoriesTool));
    registry.register(Arc::new(DeleteMemoryTool));

    // Completion
    registry.register(Arc::new(DoneTool));
}
