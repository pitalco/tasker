use anyhow::{anyhow, Result};
use base64::Engine;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::dom::{
    BackendNodeId as CdpBackendNodeId, DescribeNodeParams, FocusParams, GetBoxModelParams,
    ScrollIntoViewIfNeededParams,
};
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchMouseEventParams, DispatchMouseEventType, MouseButton,
};
use chromiumoxide::cdp::browser_protocol::page::{AddScriptToEvaluateOnNewDocumentParams, CaptureScreenshotFormat, EventFrameNavigated};
use chromiumoxide::cdp::js_protocol::runtime::{AddBindingParams, EventBindingCalled};
use chromiumoxide::listeners::EventStream;
use chromiumoxide::Page;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::browser::cdp_dom::{self, BackendNodeId, DOMExtractionResult};
use crate::models::Viewport;

/// Manages browser lifecycle and page connections
pub struct BrowserManager {
    browser: Arc<Mutex<Option<Browser>>>,
    page: Arc<Mutex<Option<Page>>>,
    /// Lock to prevent concurrent browser launches (race condition fix)
    launch_lock: tokio::sync::Mutex<()>,
}

impl BrowserManager {
    pub fn new() -> Self {
        Self {
            browser: Arc::new(Mutex::new(None)),
            page: Arc::new(Mutex::new(None)),
            launch_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Launch browser and navigate to URL
    pub async fn launch(&self, url: &str, headless: bool, viewport: Option<Viewport>) -> Result<()> {
        self.launch_with_options(url, headless, viewport, false).await
    }

    /// Launch browser in incognito mode (clean session, no cookies/history)
    pub async fn launch_incognito(&self, url: &str, headless: bool, viewport: Option<Viewport>) -> Result<()> {
        self.launch_with_options(url, headless, viewport, true).await
    }

    /// Launch browser with full options
    /// Uses the SINGLE default page that Chrome creates - no extra windows
    pub async fn launch_with_options(
        &self,
        _url: &str,  // URL not used here - caller should call navigate() after setup
        headless: bool,
        viewport: Option<Viewport>,
        _incognito: bool,  // Not using incognito - it complicates things
    ) -> Result<()> {
        // Acquire launch lock to prevent race condition (double Chrome instances)
        let _launch_guard = self.launch_lock.lock().await;

        // Close any existing browser first
        self.close().await.ok();

        let viewport = viewport.unwrap_or(Viewport {
            width: 1280,
            height: 720,
        });

        let mut config = BrowserConfig::builder()
            .window_size(viewport.width as u32, viewport.height as u32);

        if !headless {
            config = config.with_head();
        }

        // Clean launch flags - no extra windows, no extensions
        // Note: --disable-infobars is deprecated but kept for older Chrome versions
        // --enable-automation is the switch that causes the yellow banner, so we exclude it
        config = config
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--disable-infobars")
            .arg("--enable-automation=false")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-default-apps")
            .arg("--disable-extensions")
            .arg("--disable-popup-blocking")
            .arg("--disable-background-networking");

        let config = config.build().map_err(|e| anyhow!("Failed to build browser config: {}", e))?;

        // Launch browser with timeout
        let (browser, mut handler) = timeout(
            Duration::from_secs(30),
            Browser::launch(config)
        )
        .await
        .map_err(|_| anyhow!("Browser launch timeout (30s) - Chrome may not be installed or is unresponsive"))?
        .map_err(|e| anyhow!("Failed to launch browser: {}", e))?;

        // Spawn handler task
        tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                tracing::trace!("Browser event: {:?}", event);
            }
        });

        // Small delay for Chrome to fully initialize
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Use existing page if available, otherwise create one
        let page = match browser.pages().await {
            Ok(pages) if !pages.is_empty() => {
                tracing::debug!("Using existing browser page");
                pages.into_iter().next().unwrap()
            }
            _ => {
                tracing::debug!("Creating new browser page");
                browser.new_page("about:blank")
                    .await
                    .map_err(|e| anyhow!("Failed to create new page: {}", e))?
            }
        };

        // Set viewport
        let emulation_params = chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams::builder()
            .width(viewport.width as i64)
            .height(viewport.height as i64)
            .device_scale_factor(1.0)
            .mobile(false)
            .build()
            .map_err(|e| anyhow!("Failed to build viewport params: {}", e))?;

        page.execute(emulation_params)
            .await
            .map_err(|e| anyhow!("Failed to set viewport: {}", e))?;

        // Store browser and page
        *self.browser.lock().await = Some(browser);
        *self.page.lock().await = Some(page);

        tracing::info!("Browser launched (single window)");
        Ok(())
    }

    /// Get current page URL
    pub async fn current_url(&self) -> Result<String> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        page.url()
            .await
            .map_err(|e| anyhow!("Failed to get URL: {}", e))?
            .ok_or_else(|| anyhow!("URL is None"))
    }

    /// Take a screenshot of the current page
    pub async fn screenshot(&self) -> Result<String> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        let screenshot = page
            .screenshot(
                chromiumoxide::page::ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .build(),
            )
            .await
            .map_err(|e| anyhow!("Failed to take screenshot: {}", e))?;

        Ok(base64::engine::general_purpose::STANDARD.encode(screenshot))
    }

    /// Get the DOM content of the page
    pub async fn get_dom(&self) -> Result<String> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        let html = page
            .content()
            .await
            .map_err(|e| anyhow!("Failed to get DOM content: {}", e))?;

        Ok(html)
    }

    /// Navigate to a URL
    pub async fn navigate(&self, url: &str) -> Result<()> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        page.goto(url)
            .await
            .map_err(|e| anyhow!("Failed to navigate to {}: {}", url, e))?;

        Ok(())
    }

    /// Scroll the page
    pub async fn scroll(&self, x: i32, y: i32) -> Result<()> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        page.evaluate(format!("window.scrollBy({}, {})", x, y))
            .await
            .map_err(|e| anyhow!("Failed to scroll: {}", e))?;

        Ok(())
    }

    /// Execute JavaScript and return result
    pub async fn evaluate(&self, script: &str) -> Result<serde_json::Value> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        let result = page
            .evaluate(script)
            .await
            .map_err(|e| anyhow!("Failed to evaluate script: {}", e))?;

        result
            .into_value()
            .map_err(|e| anyhow!("Failed to parse script result: {}", e))
    }

    /// Get indexed interactive elements using CDP-based extraction
    /// Returns elements with backend_node_id for stable interaction
    pub async fn get_indexed_elements(&self) -> Result<DOMExtractionResult> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        cdp_dom::extract_dom(page).await
    }

    /// Click element by backend_node_id with fallback strategies
    pub async fn click_by_backend_id(&self, backend_id: BackendNodeId) -> Result<()> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        // First, try to scroll element into view
        let scroll_params = ScrollIntoViewIfNeededParams {
            node_id: None,
            backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
            object_id: None,
            rect: None,
        };
        let _ = page.execute(scroll_params).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Try geometry-based click first (GetBoxModel)
        let box_params = GetBoxModelParams {
            node_id: None,
            backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
            object_id: None,
        };

        match page.execute(box_params).await {
            Ok(box_result) => {
                let model = box_result.result.model;
                let content = model.content.inner();

                // Calculate center point (content quad is [x1,y1, x2,y2, x3,y3, x4,y4])
                let center_x = (content[0] + content[2] + content[4] + content[6]) / 4.0;
                let center_y = (content[1] + content[3] + content[5] + content[7]) / 4.0;

                // Dispatch mouse events
                let mouse_down = DispatchMouseEventParams {
                    r#type: DispatchMouseEventType::MousePressed,
                    x: center_x,
                    y: center_y,
                    button: Some(MouseButton::Left),
                    click_count: Some(1),
                    modifiers: None,
                    timestamp: None,
                    delta_x: None,
                    delta_y: None,
                    pointer_type: None,
                    buttons: None,
                    tangential_pressure: None,
                    tilt_x: None,
                    tilt_y: None,
                    twist: None,
                    force: None,
                };

                let mouse_up = DispatchMouseEventParams {
                    r#type: DispatchMouseEventType::MouseReleased,
                    x: center_x,
                    y: center_y,
                    button: Some(MouseButton::Left),
                    click_count: Some(1),
                    modifiers: None,
                    timestamp: None,
                    delta_x: None,
                    delta_y: None,
                    pointer_type: None,
                    buttons: None,
                    tangential_pressure: None,
                    tilt_x: None,
                    tilt_y: None,
                    twist: None,
                    force: None,
                };

                page.execute(mouse_down).await
                    .map_err(|e| anyhow!("Failed to dispatch mousedown: {}", e))?;
                page.execute(mouse_up).await
                    .map_err(|e| anyhow!("Failed to dispatch mouseup: {}", e))?;

                Ok(())
            }
            Err(box_err) => {
                // Fallback: Use JavaScript click via Runtime.callFunctionOn
                tracing::debug!("Box model failed for {}, trying JS click fallback: {}", backend_id, box_err);

                use chromiumoxide::cdp::browser_protocol::dom::ResolveNodeParams;
                use chromiumoxide::cdp::js_protocol::runtime::CallFunctionOnParams;

                // Resolve backend_node_id to remote object
                let resolve_params = ResolveNodeParams {
                    node_id: None,
                    backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
                    object_group: Some("click-fallback".to_string()),
                    execution_context_id: None,
                };

                let resolve_result = page.execute(resolve_params).await
                    .map_err(|e| anyhow!("Failed to resolve node {}: {}", backend_id, e))?;

                let object_id = resolve_result.result.object.object_id
                    .ok_or_else(|| anyhow!("Node {} has no object ID", backend_id))?;

                // Call click() on the element
                let call_params = CallFunctionOnParams::builder()
                    .object_id(object_id)
                    .function_declaration("function() { this.scrollIntoView({block: 'center'}); this.click(); }")
                    .build()
                    .map_err(|e| anyhow!("Failed to build call params: {}", e))?;

                page.execute(call_params).await
                    .map_err(|e| anyhow!("JS click fallback failed for {}: {}", backend_id, e))?;

                Ok(())
            }
        }
    }

    /// Focus element by backend_node_id
    pub async fn focus_by_backend_id(&self, backend_id: BackendNodeId) -> Result<()> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        let params = FocusParams {
            node_id: None,
            backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
            object_id: None,
        };

        page.execute(params).await
            .map_err(|e| anyhow!("Failed to focus element: {}", e))?;

        Ok(())
    }

    /// Type text into element by backend_node_id
    pub async fn type_by_backend_id(&self, backend_id: BackendNodeId, text: &str) -> Result<()> {
        // First focus the element
        self.focus_by_backend_id(backend_id).await?;

        // Small delay for focus
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Type using keyboard events
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        // Use insertText for reliable text input
        use chromiumoxide::cdp::browser_protocol::input::{InsertTextParams};
        let params = InsertTextParams {
            text: text.to_string(),
        };

        page.execute(params).await
            .map_err(|e| anyhow!("Failed to type text: {}", e))?;

        Ok(())
    }

    /// Scroll element into view by backend_node_id
    pub async fn scroll_to_backend_id(&self, backend_id: BackendNodeId) -> Result<()> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        let params = ScrollIntoViewIfNeededParams {
            node_id: None,
            backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
            object_id: None,
            rect: None,
        };

        page.execute(params).await
            .map_err(|e| anyhow!("Failed to scroll element into view: {}", e))?;

        Ok(())
    }

    /// Select dropdown option by backend_node_id
    /// Uses CDP to resolve the node and execute selection script
    pub async fn select_option_by_backend_id(&self, backend_id: BackendNodeId, option: &str) -> Result<()> {
        // First scroll into view
        self.scroll_to_backend_id(backend_id).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;

        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        // Resolve backend_node_id to remote object
        use chromiumoxide::cdp::browser_protocol::dom::ResolveNodeParams;
        let resolve_params = ResolveNodeParams {
            node_id: None,
            backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
            object_group: Some("dropdown-select".to_string()),
            execution_context_id: None,
        };

        let resolve_result = page.execute(resolve_params).await
            .map_err(|e| anyhow!("Failed to resolve node: {}", e))?;

        let object_id = resolve_result.result.object.object_id
            .ok_or_else(|| anyhow!("Node has no object ID"))?;

        // Call function on the resolved object to select the option
        use chromiumoxide::cdp::js_protocol::runtime::CallFunctionOnParams;
        let escaped_option = option.replace('\\', "\\\\").replace('\'', "\\'");
        let function = format!(
            r#"function() {{
                const optionValue = '{}';
                if (this.tagName !== 'SELECT') {{
                    return {{ success: false, error: 'Element is not a SELECT' }};
                }}
                for (const opt of this.options) {{
                    if (opt.value === optionValue || opt.text === optionValue ||
                        opt.value.toLowerCase() === optionValue.toLowerCase() ||
                        opt.text.toLowerCase() === optionValue.toLowerCase()) {{
                        this.value = opt.value;
                        this.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        this.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        return {{ success: true, selected: opt.text }};
                    }}
                }}
                const available = Array.from(this.options).map(o => o.text).join(', ');
                return {{ success: false, error: 'Option not found', available: available }};
            }}"#,
            escaped_option
        );

        let call_params = CallFunctionOnParams::builder()
            .object_id(object_id)
            .function_declaration(function)
            .return_by_value(true)  // Important: get actual value, not object reference
            .build()
            .map_err(|e| anyhow!("Failed to build call params: {}", e))?;

        let call_result = page.execute(call_params).await
            .map_err(|e| anyhow!("Failed to execute selection: {}", e))?;

        // Parse result
        if let Some(result) = call_result.result.result.value {
            if let Some(obj) = result.as_object() {
                if obj.get("success").and_then(|v| v.as_bool()) == Some(true) {
                    return Ok(());
                } else {
                    let error = obj.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                    let available = obj.get("available").and_then(|v| v.as_str()).unwrap_or("");
                    return Err(anyhow!("{}: {}. Available options: {}", error, option, available));
                }
            }
            // If result is not an object, log it for debugging
            tracing::warn!("Unexpected select result type: {:?}", result);
        } else {
            tracing::warn!("Select returned no value, result: {:?}", call_result.result.result);
        }

        Err(anyhow!("Failed to select option '{}' - no valid response from page", option))
    }

    /// Get element info by backend_node_id
    pub async fn describe_node(&self, backend_id: BackendNodeId) -> Result<serde_json::Value> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        let params = DescribeNodeParams {
            node_id: None,
            backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
            object_id: None,
            depth: Some(0),
            pierce: Some(false),
        };

        let result = page.execute(params).await
            .map_err(|e| anyhow!("Failed to describe node: {}", e))?;

        serde_json::to_value(&result.result.node)
            .map_err(|e| anyhow!("Failed to serialize node: {}", e))
    }

    /// Get current page title
    pub async fn get_title(&self) -> Result<String> {
        let result = self.evaluate("document.title").await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// Set up a CDP binding for instant event capture (no polling!)
    /// Returns an event stream that receives EventBindingCalled events
    pub async fn setup_event_binding(&self, binding_name: &str) -> Result<EventStream<EventBindingCalled>> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        // Add the binding so JavaScript can call it
        page.execute(AddBindingParams::new(binding_name))
            .await
            .map_err(|e| anyhow!("Failed to add binding '{}': {}", binding_name, e))?;

        // Subscribe to binding called events
        let event_stream = page.event_listener::<EventBindingCalled>().await
            .map_err(|e| anyhow!("Failed to create event listener: {}", e))?;

        tracing::debug!("CDP binding '{}' set up for instant event capture", binding_name);
        Ok(event_stream)
    }

    /// Set up a listener for page navigation events
    /// Returns an event stream that receives EventFrameNavigated events
    pub async fn setup_navigation_listener(&self) -> Result<EventStream<EventFrameNavigated>> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        let event_stream = page.event_listener::<EventFrameNavigated>().await
            .map_err(|e| anyhow!("Failed to create navigation listener: {}", e))?;

        tracing::debug!("Navigation event listener set up");
        Ok(event_stream)
    }

    /// Add a script to run on every new document (persists across navigations)
    /// This is critical for recording - ensures the script survives page navigations
    pub async fn add_script_on_new_document(&self, script: &str) -> Result<()> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        let params = AddScriptToEvaluateOnNewDocumentParams::new(script.to_string());
        page.execute(params)
            .await
            .map_err(|e| anyhow!("Failed to add script to evaluate on new document: {}", e))?;

        tracing::debug!("Added script to evaluate on every new document");
        Ok(())
    }

    /// Bring the browser window to the front (restore from minimized)
    /// Call this after recording setup is complete so user can start interacting
    pub async fn bring_to_front(&self) -> Result<()> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        page.bring_to_front()
            .await
            .map_err(|e| anyhow!("Failed to bring browser to front: {}", e))?;

        tracing::debug!("Browser window brought to front");
        Ok(())
    }

    /// Close the browser
    pub async fn close(&self) -> Result<()> {
        let mut page_guard = self.page.lock().await;
        let mut browser_guard = self.browser.lock().await;

        // Close page first
        if let Some(page) = page_guard.take() {
            let _ = page.close().await;
        }

        // Then close browser
        if let Some(mut browser) = browser_guard.take() {
            let _ = browser.close().await;
        }

        tracing::info!("Browser closed");
        Ok(())
    }

    /// Get the underlying page for advanced operations
    pub async fn page(&self) -> Option<Page> {
        self.page.lock().await.clone()
    }

    /// Click element by CSS selector
    pub async fn click(&self, selector: &str) -> Result<()> {
        // Use JavaScript to click the element directly
        let script = format!(
            r#"
            (function() {{
                const el = document.querySelector({:?});
                if (!el) {{
                    throw new Error('Element not found: ' + {:?});
                }}
                el.scrollIntoView({{ behavior: 'instant', block: 'center' }});
                const rect = el.getBoundingClientRect();
                const x = rect.left + rect.width / 2;
                const y = rect.top + rect.height / 2;
                
                // Dispatch mouse events
                el.dispatchEvent(new MouseEvent('mousedown', {{
                    bubbles: true,
                    cancelable: true,
                    view: window,
                    clientX: x,
                    clientY: y,
                    button: 0
                }}));
                el.dispatchEvent(new MouseEvent('mouseup', {{
                    bubbles: true,
                    cancelable: true,
                    view: window,
                    clientX: x,
                    clientY: y,
                    button: 0
                }}));
                el.dispatchEvent(new MouseEvent('click', {{
                    bubbles: true,
                    cancelable: true,
                    view: window,
                    clientX: x,
                    clientY: y,
                    button: 0
                }}));
                return true;
            }})()
            "#,
            selector, selector
        );

        self.evaluate(&script)
            .await
            .map_err(|e| anyhow!("Failed to click element '{}': {}", selector, e))?;

        Ok(())
    }

    /// Type text into element by CSS selector
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<()> {
        // Use JavaScript to type into the element
        // We need to trigger events in a way that the recording script will capture
        let script = format!(
            r#"
            (function() {{
                const el = document.querySelector({:?});
                if (!el) {{
                    throw new Error('Element not found: ' + {:?});
                }}
                // Focus first
                el.focus();
                
                // Clear existing value
                el.value = '';
                
                // Set new value
                el.value = {:?};
                
                // Trigger input event (this is what the recording script listens for)
                el.dispatchEvent(new Event('input', {{ bubbles: true, cancelable: true }}));
                
                // Small delay to let debounce timer start, then trigger blur to force immediate capture
                setTimeout(() => {{
                    el.dispatchEvent(new Event('blur', {{ bubbles: true, cancelable: true }}));
                }}, 50);
                
                return true;
            }})()
            "#,
            selector, selector, text
        );

        self.evaluate(&script)
            .await
            .map_err(|e| anyhow!("Failed to type text into element '{}': {}", selector, e))?;

        // Wait a bit for the blur event to fire and be captured
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    /// Select option in dropdown by CSS selector
    pub async fn select(&self, selector: &str, value: &str) -> Result<()> {
        // Use JavaScript to set the select value
        let script = format!(
            r#"
            (function() {{
                const el = document.querySelector({:?});
                if (!el) {{
                    throw new Error('Element not found');
                }}
                if (el.tagName !== 'SELECT') {{
                    throw new Error('Element is not a SELECT');
                }}
                el.value = {:?};
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                return true;
            }})()
            "#,
            selector, value
        );

        self.evaluate(&script)
            .await
            .map_err(|e| anyhow!("Failed to select option '{}' in '{}': {}", value, selector, e))?;

        Ok(())
    }
}

impl Default for BrowserManager {
    fn default() -> Self {
        Self::new()
    }
}
