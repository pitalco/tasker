use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::registry::{Tool, ToolContext, ToolDefinition, ToolResult};

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
        ctx.browser.evaluate("window.history.back()").await?;
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
                        "minimum": 1,
                        "maximum": 30
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let seconds = params["seconds"].as_i64().unwrap_or(3).clamp(1, 30);
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
        let index = params["index"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'index' parameter"))? as i32;

        // Look up backend_node_id from selector map
        let selector_map = ctx.selector_map.read().await;
        let backend_id = match selector_map.get_backend_id(index) {
            Some(id) => id,
            None => return Ok(ToolResult::error(format!(
                "Element index {} not found. Valid indices: 1-{}",
                index, selector_map.len()
            ))),
        };
        drop(selector_map); // Release read lock before async operations

        match ctx.browser.click_by_backend_id(backend_id).await {
            Ok(()) => Ok(ToolResult::success(format!("Clicked element [{}]", index))),
            Err(e) => Ok(ToolResult::error(format!("Failed to click element [{}]: {}", index, e)))
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
            description: "Type text into an input field identified by index. Clears existing content first.".to_string(),
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
        let index = params["index"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'index' parameter"))? as i32;
        let text = params["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'text' parameter"))?;

        // Look up backend_node_id from selector map
        let selector_map = ctx.selector_map.read().await;
        let backend_id = match selector_map.get_backend_id(index) {
            Some(id) => id,
            None => return Ok(ToolResult::error(format!(
                "Element index {} not found. Valid indices: 1-{}",
                index, selector_map.len()
            ))),
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
        if let Some(index) = params["to_element"].as_i64() {
            let index = index as i32;
            // Look up element from selector map
            let selector_map = ctx.selector_map.read().await;
            let element = match selector_map.get_element_by_index(index) {
                Some(el) => el.clone(),
                None => return Ok(ToolResult::error(format!(
                    "Element index {} not found. Valid indices: 1-{}",
                    index, selector_map.len()
                ))),
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
            description: "Send keyboard keys like Enter, Tab, Escape, or key combinations".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "keys": {
                        "type": "string",
                        "description": "Keys to send (e.g., 'Enter', 'Tab', 'Escape', 'Control+a')"
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

        // Map common key names to their JS keycode
        let script = format!(
            r#"
            const keyMap = {{
                'Enter': {{ key: 'Enter', code: 'Enter', keyCode: 13 }},
                'Tab': {{ key: 'Tab', code: 'Tab', keyCode: 9 }},
                'Escape': {{ key: 'Escape', code: 'Escape', keyCode: 27 }},
                'Backspace': {{ key: 'Backspace', code: 'Backspace', keyCode: 8 }},
                'Delete': {{ key: 'Delete', code: 'Delete', keyCode: 46 }},
                'ArrowUp': {{ key: 'ArrowUp', code: 'ArrowUp', keyCode: 38 }},
                'ArrowDown': {{ key: 'ArrowDown', code: 'ArrowDown', keyCode: 40 }},
                'ArrowLeft': {{ key: 'ArrowLeft', code: 'ArrowLeft', keyCode: 37 }},
                'ArrowRight': {{ key: 'ArrowRight', code: 'ArrowRight', keyCode: 39 }},
            }};
            const keyInfo = keyMap['{}'] || {{ key: '{}', code: 'Key' + '{}', keyCode: '{}'.charCodeAt(0) }};
            document.activeElement.dispatchEvent(new KeyboardEvent('keydown', {{ ...keyInfo, bubbles: true }}));
            document.activeElement.dispatchEvent(new KeyboardEvent('keyup', {{ ...keyInfo, bubbles: true }}));
            return 'sent';
            "#,
            keys, keys, keys, keys
        );

        ctx.browser.evaluate(&script).await?;
        Ok(ToolResult::success(format!("Sent keys: {}", keys)))
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
            r#"
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
            "#,
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
        let index = params["index"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'index' parameter"))? as i32;
        let option = params["option"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'option' parameter"))?;

        // Look up element from selector map
        let selector_map = ctx.selector_map.read().await;
        let element = match selector_map.get_element_by_index(index) {
            Some(el) => el.clone(),
            None => return Ok(ToolResult::error(format!(
                "Element index {} not found. Valid indices: 1-{}",
                index, selector_map.len()
            ))),
        };
        drop(selector_map);

        // Check if element is a select
        if element.tag.to_lowercase() != "select" {
            return Ok(ToolResult::error(format!(
                "Element [{}] is not a select/dropdown (found {})",
                index, element.tag
            )));
        }

        // Use JavaScript to select option by clicking the element position
        let script = format!(
            r#"
            const elements = document.elementsFromPoint({}, {});
            for (const el of elements) {{
                if (el.tagName === 'SELECT') {{
                    const optionValue = '{}';
                    for (const opt of el.options) {{
                        if (opt.value === optionValue || opt.text === optionValue) {{
                            el.value = opt.value;
                            el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                            return 'selected';
                        }}
                    }}
                    return 'option not found';
                }}
            }}
            return 'select not found';
            "#,
            element.bounds.x + element.bounds.width / 2.0,
            element.bounds.y + element.bounds.height / 2.0,
            option.replace('\'', "\\'")
        );

        match ctx.browser.evaluate(&script).await? {
            result if result.as_str() == Some("selected") => Ok(ToolResult::success(format!(
                "Selected option '{}' in dropdown [{}]",
                option, index
            ))),
            result => Ok(ToolResult::error(format!(
                "Failed to select option '{}' in dropdown [{}]: {:?}",
                option, index, result
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
        let index = params["index"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'index' parameter"))? as i32;

        // Look up element from selector map
        let selector_map = ctx.selector_map.read().await;
        let element = match selector_map.get_element_by_index(index) {
            Some(el) => el.clone(),
            None => return Ok(ToolResult::error(format!(
                "Element index {} not found. Valid indices: 1-{}",
                index, selector_map.len()
            ))),
        };
        drop(selector_map);

        // Check if element is a select
        if element.tag.to_lowercase() != "select" {
            return Ok(ToolResult::error(format!(
                "Element [{}] is not a select/dropdown (found {})",
                index, element.tag
            )));
        }

        // Use JavaScript to get options at element position
        let script = format!(
            r#"
            const elements = document.elementsFromPoint({}, {});
            for (const el of elements) {{
                if (el.tagName === 'SELECT') {{
                    return JSON.stringify(Array.from(el.options).map(o => ({{ value: o.value, text: o.text, selected: o.selected }})));
                }}
            }}
            return JSON.stringify([]);
            "#,
            element.bounds.x + element.bounds.width / 2.0,
            element.bounds.y + element.bounds.height / 2.0
        );

        let result = ctx.browser.evaluate(&script).await?;
        let options: Vec<Value> = serde_json::from_str(result.as_str().unwrap_or("[]")).unwrap_or_default();

        if options.is_empty() {
            Ok(ToolResult::error(format!("No dropdown found at index [{}] or no options available", index)))
        } else {
            Ok(ToolResult::success_with_data(
                format!("Found {} options in dropdown [{}]", options.len(), index),
                json!({ "options": options })
            ))
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
        let index = params["index"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'index' parameter"))?;
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

/// Switch to a different tab
pub struct SwitchTabTool;

#[async_trait]
impl Tool for SwitchTabTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "switch_tab".to_string(),
            description: "Switch to a different browser tab by index".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "tab_index": {
                        "type": "integer",
                        "description": "The index of the tab to switch to (0-based)"
                    }
                },
                "required": ["tab_index"]
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let tab_index = params["tab_index"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'tab_index' parameter"))?;

        // Tab switching requires access to browser's page list - placeholder
        Ok(ToolResult::error(format!(
            "Tab switching to index {} - feature requires multi-tab browser manager",
            tab_index
        )))
    }
}

/// Close current or specified tab
pub struct CloseTabTool;

#[async_trait]
impl Tool for CloseTabTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "close_tab".to_string(),
            description: "Close the current tab or a specific tab by index".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "tab_index": {
                        "type": "integer",
                        "description": "The index of the tab to close. If not specified, closes current tab."
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let tab_index = params["tab_index"].as_i64();

        Ok(ToolResult::error(format!(
            "Tab closing {:?} - feature requires multi-tab browser manager",
            tab_index
        )))
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
// File Tools (for agent context, not browser)
// ============================================================================

/// Read a local file
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a local file".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file to read"
                    }
                },
                "required": ["file_path"]
            }),
        }
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let file_path = params["file_path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'file_path' parameter"))?;

        match tokio::fs::read_to_string(file_path).await {
            Ok(content) => {
                let preview = if content.len() > 10000 {
                    format!("{}...\n[Truncated. Total {} chars]", &content[..10000], content.len())
                } else {
                    content
                };
                Ok(ToolResult::success_with_data(
                    format!("Read file: {}", file_path),
                    json!({ "content": preview })
                ))
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to read file '{}': {}", file_path, e)))
        }
    }
}

/// Write to a local file
pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write_file".to_string(),
            description: "Write content to a local file (creates or overwrites)".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file to write"
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

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let file_path = params["file_path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'file_path' parameter"))?;
        let content = params["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'content' parameter"))?;

        match tokio::fs::write(file_path, content).await {
            Ok(()) => Ok(ToolResult::success(format!(
                "Wrote {} bytes to '{}'",
                content.len(),
                file_path
            ))),
            Err(e) => Ok(ToolResult::error(format!("Failed to write file '{}': {}", file_path, e)))
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
            description: "Find and replace text in a file".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file"
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

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let file_path = params["file_path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'file_path' parameter"))?;
        let find = params["find"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'find' parameter"))?;
        let replace = params["replace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'replace' parameter"))?;

        match tokio::fs::read_to_string(file_path).await {
            Ok(content) => {
                let count = content.matches(find).count();
                if count == 0 {
                    return Ok(ToolResult::error(format!("Text '{}' not found in file", find)));
                }
                let new_content = content.replace(find, replace);
                match tokio::fs::write(file_path, &new_content).await {
                    Ok(()) => Ok(ToolResult::success(format!(
                        "Replaced {} occurrences in '{}'",
                        count, file_path
                    ))),
                    Err(e) => Ok(ToolResult::error(format!("Failed to write file: {}", e)))
                }
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to read file '{}': {}", file_path, e)))
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
    registry.register(Arc::new(InputTextTool));
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
    registry.register(Arc::new(SwitchTabTool));
    registry.register(Arc::new(CloseTabTool));

    // JavaScript
    registry.register(Arc::new(EvaluateJsTool));

    // Files
    registry.register(Arc::new(ReadFileTool));
    registry.register(Arc::new(WriteFileTool));
    registry.register(Arc::new(ReplaceInFileTool));

    // Completion
    registry.register(Arc::new(DoneTool));
}
