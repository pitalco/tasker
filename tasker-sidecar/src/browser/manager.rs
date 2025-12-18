use anyhow::{anyhow, Context, Result};
use base64::Engine;
use chromiumoxide::browser::{Browser, BrowserConfig};
use image::GenericImageView;
use chromiumoxide::cdp::browser_protocol::dom::{
    BackendNodeId as CdpBackendNodeId, DescribeNodeParams, FocusParams, GetBoxModelParams,
    ResolveNodeParams, ScrollIntoViewIfNeededParams,
};
use chromiumoxide::cdp::js_protocol::runtime::CallFunctionOnParams;
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchKeyEventParams, DispatchKeyEventType,
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
use tracing::instrument;

use crate::browser::cdp_dom::{self, BackendNodeId, DOMExtractionResult};
use crate::models::Viewport;

/// Manages browser lifecycle and page connections
pub struct BrowserManager {
    browser: Arc<Mutex<Option<Browser>>>,
    /// Multiple tabs/pages
    pages: Arc<Mutex<Vec<Page>>>,
    /// Currently active tab index
    active_tab: Arc<Mutex<usize>>,
    /// Lock to prevent concurrent browser launches (race condition fix)
    launch_lock: tokio::sync::Mutex<()>,
    /// Whether browser is running in headless mode
    headless: Arc<Mutex<bool>>,
}

impl BrowserManager {
    pub fn new() -> Self {
        Self {
            browser: Arc::new(Mutex::new(None)),
            pages: Arc::new(Mutex::new(Vec::new())),
            active_tab: Arc::new(Mutex::new(0)),
            launch_lock: tokio::sync::Mutex::new(()),
            headless: Arc::new(Mutex::new(false)),
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
    #[instrument(skip(self), fields(headless = headless))]
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

        // Use disable_default_args() to prevent chromiumoxide from adding --enable-automation
        // which causes the yellow "Chrome is being controlled" banner
        let mut config = BrowserConfig::builder()
            .disable_default_args();

        if headless {
            // For headless mode, set a fixed viewport size
            config = config.window_size(viewport.width as u32, viewport.height as u32);
        } else {
            // For headed mode, start maximized and let Chrome determine viewport
            config = config
                .with_head()
                .arg("--start-maximized");
        }

        // Manually add chromiumoxide's DEFAULT_ARGS, EXCEPT --enable-automation
        // This removes the automation banner while keeping other useful defaults
        config = config
            .arg("--disable-background-networking")
            .arg("--enable-features=NetworkService,NetworkServiceInProcess")
            .arg("--disable-background-timer-throttling")
            .arg("--disable-backgrounding-occluded-windows")
            .arg("--disable-breakpad")
            .arg("--disable-client-side-phishing-detection")
            .arg("--disable-component-extensions-with-background-pages")
            .arg("--disable-default-apps")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-features=TranslateUI")
            .arg("--disable-hang-monitor")
            .arg("--disable-ipc-flooding-protection")
            .arg("--disable-popup-blocking")
            .arg("--disable-prompt-on-repost")
            .arg("--disable-renderer-backgrounding")
            .arg("--disable-sync")
            .arg("--force-color-profile=srgb")
            .arg("--metrics-recording-only")
            .arg("--no-first-run")
            .arg("--password-store=basic")
            .arg("--use-mock-keychain")
            .arg("--lang=en_US")
            .arg("--disable-infobars")
            .arg("--no-default-browser-check")
            .arg("--disable-extensions");

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

        // Configure viewport based on mode
        if headless {
            // For headless mode, set explicit viewport size
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
        } else {
            // For headed mode, explicitly CLEAR any viewport emulation
            // This ensures the browser uses its natural maximized window size
            use chromiumoxide::cdp::browser_protocol::emulation::ClearDeviceMetricsOverrideParams;
            page.execute(ClearDeviceMetricsOverrideParams::default())
                .await
                .ok(); // Ignore errors - this is best-effort cleanup
        }

        // Store browser, page, and headless state
        *self.browser.lock().await = Some(browser);
        *self.headless.lock().await = headless;
        let mut pages = self.pages.lock().await;
        pages.clear(); // Clear any old pages
        pages.push(page);
        *self.active_tab.lock().await = 0;

        tracing::info!("Browser launched (single window, headless={})", headless);
        Ok(())
    }

    /// Get the active page (internal helper)
    async fn get_active_page(&self) -> Result<Page> {
        let pages = self.pages.lock().await;
        let active = *self.active_tab.lock().await;
        pages.get(active)
            .cloned()
            .ok_or_else(|| anyhow!("No active tab (index {} of {} tabs)", active, pages.len()))
    }

    /// Open a new tab and switch to it
    pub async fn new_tab(&self, url: &str) -> Result<usize> {
        let browser_guard = self.browser.lock().await;
        let browser = browser_guard.as_ref().ok_or_else(|| anyhow!("No browser running"))?;

        let page = browser.new_page(url).await
            .map_err(|e| anyhow!("Failed to create new tab: {}", e))?;

        // For headed mode, clear any viewport emulation so tab uses natural window size
        let is_headless = *self.headless.lock().await;
        if !is_headless {
            use chromiumoxide::cdp::browser_protocol::emulation::ClearDeviceMetricsOverrideParams;
            page.execute(ClearDeviceMetricsOverrideParams::default())
                .await
                .ok(); // Ignore errors - best-effort cleanup
        }

        let mut pages = self.pages.lock().await;
        pages.push(page);
        let tab_index = pages.len() - 1;
        drop(pages);

        *self.active_tab.lock().await = tab_index;
        tracing::info!("Opened new tab {} at {}", tab_index, url);
        Ok(tab_index)
    }

    /// Switch to tab by index
    pub async fn switch_tab(&self, index: usize) -> Result<()> {
        let pages = self.pages.lock().await;
        if index >= pages.len() {
            return Err(anyhow!("Tab index {} out of range (have {} tabs)", index, pages.len()));
        }
        drop(pages);

        *self.active_tab.lock().await = index;
        tracing::info!("Switched to tab {}", index);
        Ok(())
    }

    /// Close tab by index
    pub async fn close_tab(&self, index: usize) -> Result<()> {
        let mut pages = self.pages.lock().await;
        if index >= pages.len() {
            return Err(anyhow!("Tab index {} out of range", index));
        }
        if pages.len() == 1 {
            return Err(anyhow!("Cannot close last tab"));
        }

        let page = pages.remove(index);
        let _ = page.close().await;

        // Adjust active tab if needed
        let mut active = self.active_tab.lock().await;
        if *active >= pages.len() {
            *active = pages.len() - 1;
        }

        tracing::info!("Closed tab {}, active is now {}", index, *active);
        Ok(())
    }

    /// List open tabs (returns tab index and URL)
    pub async fn list_tabs(&self) -> Result<Vec<(usize, String)>> {
        let pages = self.pages.lock().await;
        let mut tabs = Vec::new();
        for (i, page) in pages.iter().enumerate() {
            let url = page.url().await
                .map_err(|e| anyhow!("Failed to get URL for tab {}: {}", i, e))?
                .unwrap_or_default();
            tabs.push((i, url));
        }
        Ok(tabs)
    }

    /// Get active tab index
    pub async fn active_tab_index(&self) -> usize {
        *self.active_tab.lock().await
    }

    /// Get current page URL
    pub async fn current_url(&self) -> Result<String> {
        let page = self.get_active_page().await?;
        page.url()
            .await
            .map_err(|e| anyhow!("Failed to get URL: {}", e))?
            .ok_or_else(|| anyhow!("URL is None"))
    }

    /// Take a screenshot of the current page, resized to reduce token usage
    pub async fn screenshot(&self) -> Result<String> {
        let page = self.get_active_page().await?;

        // Capture as PNG first (lossless for resizing)
        let screenshot_bytes = page
            .screenshot(
                chromiumoxide::page::ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .build(),
            )
            .await
            .map_err(|e| anyhow!("Failed to take screenshot: {}", e))?;

        // Load and resize the image to reduce token usage
        // Target: 1280px wide (or less), maintaining aspect ratio
        let img = image::load_from_memory(&screenshot_bytes)
            .map_err(|e| anyhow!("Failed to decode screenshot: {}", e))?;

        let (width, height) = img.dimensions();
        let max_width = 1280u32;

        let resized = if width > max_width {
            let scale = max_width as f32 / width as f32;
            let new_height = (height as f32 * scale) as u32;
            img.resize(max_width, new_height, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };

        // Encode as JPEG with good quality
        let mut jpeg_bytes = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut jpeg_bytes);
        resized.write_to(&mut cursor, image::ImageFormat::Jpeg)
            .map_err(|e| anyhow!("Failed to encode screenshot as JPEG: {}", e))?;

        Ok(base64::engine::general_purpose::STANDARD.encode(jpeg_bytes))
    }

    /// Get the DOM content of the page
    pub async fn get_dom(&self) -> Result<String> {
        let page = self.get_active_page().await?;
        let html = page
            .content()
            .await
            .map_err(|e| anyhow!("Failed to get DOM content: {}", e))?;

        Ok(html)
    }

    /// Navigate to a URL
    #[instrument(skip(self), fields(url = %url))]
    pub async fn navigate(&self, url: &str) -> Result<()> {
        let page = self.get_active_page().await
            .context("Failed to get active page for navigation")?;
        page.goto(url)
            .await
            .with_context(|| format!("Failed to navigate to {}", url))?;

        Ok(())
    }

    /// Scroll the page using CDP mouse wheel events (more reliable than JS)
    pub async fn scroll(&self, x: i32, y: i32) -> Result<()> {
        let page = self.get_active_page().await?;

        // Use CDP Input.dispatchMouseEvent with mouseWheel type
        let scroll_event = DispatchMouseEventParams {
            r#type: DispatchMouseEventType::MouseWheel,
            x: 400.0,  // Center of viewport
            y: 300.0,
            button: None,
            click_count: None,
            modifiers: None,
            timestamp: None,
            delta_x: Some(x as f64),
            delta_y: Some(y as f64),
            pointer_type: None,
            buttons: None,
            tangential_pressure: None,
            tilt_x: None,
            tilt_y: None,
            twist: None,
            force: None,
        };

        page.execute(scroll_event).await
            .map_err(|e| anyhow!("Failed to scroll: {}", e))?;

        Ok(())
    }

    /// Execute JavaScript and return result
    pub async fn evaluate(&self, script: &str) -> Result<serde_json::Value> {
        let page = self.get_active_page().await?;
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
    /// Waits for page to be ready and retries if no elements found
    #[instrument(skip(self))]
    pub async fn get_indexed_elements(&self) -> Result<DOMExtractionResult> {
        let page = self.get_active_page().await
            .context("Failed to get active page for DOM extraction")?;

        // Wait for document.readyState to be "complete" (max 3 seconds)
        // Most pages load faster, so we don't need to wait too long
        for _ in 0..30 {
            let ready_state: String = page
                .evaluate("document.readyState")
                .await
                .map(|v| v.into_value().unwrap_or_default())
                .unwrap_or_default();

            if ready_state == "complete" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Extract elements with retry for dynamic content
        // Use exponential backoff: 50ms, 100ms, 200ms (max ~350ms total wait)
        const MAX_RETRIES: u32 = 3;
        let mut backoff_ms = 50u64;

        for attempt in 0..MAX_RETRIES {
            let result = cdp_dom::extract_dom(&page).await?;

            // Success if we have any interactive elements
            if !result.selector_map.ordered_elements.is_empty() {
                tracing::debug!(
                    "DOM extraction succeeded on attempt {} with {} elements",
                    attempt + 1,
                    result.selector_map.ordered_elements.len()
                );
                return Ok(result);
            }

            // Don't sleep on last attempt
            if attempt < MAX_RETRIES - 1 {
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms *= 2; // Exponential backoff
            }
        }

        // Return last result even if empty (let caller handle it)
        // This allows the agent to see the page state even without interactive elements
        tracing::warn!("DOM extraction found no interactive elements after {} retries", MAX_RETRIES);
        cdp_dom::extract_dom(&page).await
    }

    /// Click element by backend_node_id with fallback strategies
    #[instrument(skip(self), fields(backend_id = backend_id))]
    pub async fn click_by_backend_id(&self, backend_id: BackendNodeId) -> Result<()> {
        let page = self.get_active_page().await
            .context("Failed to get active page for click")?;

        // First, try to scroll element into view
        let scroll_params = ScrollIntoViewIfNeededParams {
            node_id: None,
            backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
            object_id: None,
            rect: None,
        };
        let _ = page.execute(scroll_params).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Highlight element with yellow flickering border before clicking
        self.highlight_element(&page, backend_id).await;

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

    /// Hover over element by backend_node_id to trigger hover states/tooltips
    #[instrument(skip(self), fields(backend_id = backend_id))]
    pub async fn hover_by_backend_id(&self, backend_id: BackendNodeId) -> Result<()> {
        let page = self.get_active_page().await
            .context("Failed to get active page for hover")?;

        // First, try to scroll element into view
        let scroll_params = ScrollIntoViewIfNeededParams {
            node_id: None,
            backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
            object_id: None,
            rect: None,
        };
        let _ = page.execute(scroll_params).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Highlight element with yellow flickering border
        self.highlight_element(&page, backend_id).await;

        // Get element geometry
        let box_params = GetBoxModelParams {
            node_id: None,
            backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
            object_id: None,
        };

        match page.execute(box_params).await {
            Ok(box_result) => {
                let model = box_result.result.model;
                let content = model.content.inner();

                // Calculate center point
                let center_x = (content[0] + content[2] + content[4] + content[6]) / 4.0;
                let center_y = (content[1] + content[3] + content[5] + content[7]) / 4.0;

                // Dispatch mouse move event to trigger hover
                let mouse_move = DispatchMouseEventParams {
                    r#type: DispatchMouseEventType::MouseMoved,
                    x: center_x,
                    y: center_y,
                    button: None,
                    click_count: None,
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

                page.execute(mouse_move).await
                    .map_err(|e| anyhow!("Failed to dispatch mousemove: {}", e))?;

                // Small delay to let hover effects appear
                tokio::time::sleep(Duration::from_millis(100)).await;

                Ok(())
            }
            Err(box_err) => {
                // Fallback: Use JavaScript to trigger hover events
                tracing::debug!("Box model failed for {}, trying JS hover fallback: {}", backend_id, box_err);

                let resolve_params = ResolveNodeParams {
                    node_id: None,
                    backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
                    object_group: Some("hover-fallback".to_string()),
                    execution_context_id: None,
                };

                let resolve_result = page.execute(resolve_params).await
                    .map_err(|e| anyhow!("Failed to resolve node {}: {}", backend_id, e))?;

                let object_id = resolve_result.result.object.object_id
                    .ok_or_else(|| anyhow!("Node {} has no object ID", backend_id))?;

                // Dispatch mouseenter and mouseover events via JavaScript
                let call_params = CallFunctionOnParams::builder()
                    .object_id(object_id)
                    .function_declaration("function() { this.scrollIntoView({block: 'center'}); this.dispatchEvent(new MouseEvent('mouseenter', {bubbles: true})); this.dispatchEvent(new MouseEvent('mouseover', {bubbles: true})); }")
                    .build()
                    .map_err(|e| anyhow!("Failed to build call params: {}", e))?;

                page.execute(call_params).await
                    .map_err(|e| anyhow!("JS hover fallback failed for {}: {}", backend_id, e))?;

                tokio::time::sleep(Duration::from_millis(100)).await;

                Ok(())
            }
        }
    }

    /// Highlight element with yellow flickering border (visual feedback before click)
    /// This is non-blocking - the highlight animation runs in the browser while we continue
    async fn highlight_element(&self, page: &Page, backend_id: BackendNodeId) {
        // Resolve backend_node_id to JavaScript object
        let resolve_params = ResolveNodeParams {
            node_id: None,
            backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
            object_group: Some("highlight".to_string()),
            execution_context_id: None,
        };

        if let Ok(result) = page.execute(resolve_params).await {
            if let Some(object_id) = result.result.object.object_id {
                // Reduced highlight: single brief flash instead of 6 flickers
                // This provides visual feedback without blocking for 650ms
                let highlight_js = r#"
                function() {
                    const orig = {
                        outline: this.style.outline,
                        outlineOffset: this.style.outlineOffset,
                        transition: this.style.transition
                    };

                    this.style.transition = 'outline-color 0.15s ease-out';
                    this.style.outline = '3px solid yellow';
                    this.style.outlineOffset = '2px';

                    // Single flash - restore after 200ms
                    setTimeout(() => {
                        this.style.outline = orig.outline;
                        this.style.outlineOffset = orig.outlineOffset;
                        this.style.transition = orig.transition;
                    }, 200);
                }
                "#;

                let call_params = CallFunctionOnParams::builder()
                    .object_id(object_id)
                    .function_declaration(highlight_js)
                    .build();

                if let Ok(params) = call_params {
                    let _ = page.execute(params).await;
                    // Only wait 50ms for the highlight to start - don't wait for it to complete
                    // The animation runs asynchronously in the browser
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        }
    }

    /// Focus element by backend_node_id
    pub async fn focus_by_backend_id(&self, backend_id: BackendNodeId) -> Result<()> {
        let page = self.get_active_page().await?;

        let params = FocusParams {
            node_id: None,
            backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
            object_id: None,
        };

        page.execute(params).await
            .map_err(|e| anyhow!("Failed to focus element: {}", e))?;

        Ok(())
    }

    /// Clear an input field by backend_node_id (select all + delete)
    pub async fn clear_input_by_backend_id(&self, backend_id: BackendNodeId) -> Result<()> {
        // Click to focus (works on any clickable element, not just natively focusable ones)
        self.click_by_backend_id(backend_id).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;

        let page = self.get_active_page().await?;

        // Select all (Ctrl+A)
        let ctrl_a_down = DispatchKeyEventParams {
            r#type: DispatchKeyEventType::KeyDown,
            modifiers: Some(2), // Ctrl modifier
            key: Some("a".to_string()),
            code: Some("KeyA".to_string()),
            windows_virtual_key_code: Some(65),
            text: None,
            unmodified_text: None,
            key_identifier: None,
            native_virtual_key_code: None,
            auto_repeat: None,
            is_keypad: None,
            is_system_key: None,
            location: None,
            timestamp: None,
            commands: None,
        };
        page.execute(ctrl_a_down).await.ok();

        let ctrl_a_up = DispatchKeyEventParams {
            r#type: DispatchKeyEventType::KeyUp,
            modifiers: Some(2),
            key: Some("a".to_string()),
            code: Some("KeyA".to_string()),
            windows_virtual_key_code: Some(65),
            text: None,
            unmodified_text: None,
            key_identifier: None,
            native_virtual_key_code: None,
            auto_repeat: None,
            is_keypad: None,
            is_system_key: None,
            location: None,
            timestamp: None,
            commands: None,
        };
        page.execute(ctrl_a_up).await.ok();

        tokio::time::sleep(Duration::from_millis(30)).await;

        // Delete the selection
        let delete_down = DispatchKeyEventParams {
            r#type: DispatchKeyEventType::KeyDown,
            modifiers: None,
            key: Some("Backspace".to_string()),
            code: Some("Backspace".to_string()),
            windows_virtual_key_code: Some(8),
            text: None,
            unmodified_text: None,
            key_identifier: None,
            native_virtual_key_code: None,
            auto_repeat: None,
            is_keypad: None,
            is_system_key: None,
            location: None,
            timestamp: None,
            commands: None,
        };
        page.execute(delete_down).await.ok();

        let delete_up = DispatchKeyEventParams {
            r#type: DispatchKeyEventType::KeyUp,
            modifiers: None,
            key: Some("Backspace".to_string()),
            code: Some("Backspace".to_string()),
            windows_virtual_key_code: Some(8),
            text: None,
            unmodified_text: None,
            key_identifier: None,
            native_virtual_key_code: None,
            auto_repeat: None,
            is_keypad: None,
            is_system_key: None,
            location: None,
            timestamp: None,
            commands: None,
        };
        page.execute(delete_up).await.ok();

        Ok(())
    }

    /// Type text into element by backend_node_id
    pub async fn type_by_backend_id(&self, backend_id: BackendNodeId, text: &str) -> Result<()> {
        // First focus the element
        self.focus_by_backend_id(backend_id).await?;

        // Small delay for focus
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Type using keyboard events
        let page = self.get_active_page().await?;

        // Use insertText for reliable text input
        use chromiumoxide::cdp::browser_protocol::input::{InsertTextParams};
        let params = InsertTextParams {
            text: text.to_string(),
        };

        page.execute(params).await
            .map_err(|e| anyhow!("Failed to type text: {}", e))?;

        Ok(())
    }

    /// Press a key using CDP Input.dispatchKeyEvent (more reliable than JS events)
    pub async fn press_key(&self, key: &str) -> Result<()> {
        let page = self.get_active_page().await?;

        // Map key names to CDP key codes and text
        let (key_code, code, text, key_name) = match key.to_lowercase().as_str() {
            "enter" | "return" => (13, "Enter", "\r", "Enter"),
            "tab" => (9, "Tab", "", "Tab"),
            "escape" | "esc" => (27, "Escape", "", "Escape"),
            "backspace" => (8, "Backspace", "", "Backspace"),
            "delete" => (46, "Delete", "", "Delete"),
            "arrowup" | "up" => (38, "ArrowUp", "", "ArrowUp"),
            "arrowdown" | "down" => (40, "ArrowDown", "", "ArrowDown"),
            "arrowleft" | "left" => (37, "ArrowLeft", "", "ArrowLeft"),
            "arrowright" | "right" => (39, "ArrowRight", "", "ArrowRight"),
            "space" => (32, "Space", " ", " "),
            "home" => (36, "Home", "", "Home"),
            "end" => (35, "End", "", "End"),
            "pageup" => (33, "PageUp", "", "PageUp"),
            "pagedown" => (34, "PageDown", "", "PageDown"),
            _ => {
                // For single characters, use their char code
                if key.len() == 1 {
                    let c = key.chars().next().unwrap();
                    (c as i64, &format!("Key{}", c.to_uppercase())[..], key, key)
                } else {
                    return Err(anyhow!("Unknown key: {}", key));
                }
            }
        };

        // Key down event
        let key_down = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyDown)
            .key(key_name)
            .code(code)
            .windows_virtual_key_code(key_code)
            .native_virtual_key_code(key_code);

        let key_down = if !text.is_empty() {
            key_down.text(text).build()
        } else {
            key_down.build()
        }.map_err(|e| anyhow!("Failed to build key down params: {}", e))?;

        page.execute(key_down).await
            .map_err(|e| anyhow!("Failed to dispatch key down: {}", e))?;

        // Key up event
        let key_up = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyUp)
            .key(key_name)
            .code(code)
            .windows_virtual_key_code(key_code)
            .native_virtual_key_code(key_code)
            .build()
            .map_err(|e| anyhow!("Failed to build key up params: {}", e))?;

        page.execute(key_up).await
            .map_err(|e| anyhow!("Failed to dispatch key up: {}", e))?;

        Ok(())
    }

    /// Scroll element into view by backend_node_id
    pub async fn scroll_to_backend_id(&self, backend_id: BackendNodeId) -> Result<()> {
        let page = self.get_active_page().await?;

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

        let page = self.get_active_page().await?;

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
        let page = self.get_active_page().await?;

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
        let page = self.get_active_page().await?;

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
        let page = self.get_active_page().await?;

        let event_stream = page.event_listener::<EventFrameNavigated>().await
            .map_err(|e| anyhow!("Failed to create navigation listener: {}", e))?;

        tracing::debug!("Navigation event listener set up");
        Ok(event_stream)
    }

    /// Add a script to run on every new document (persists across navigations)
    /// This is critical for recording - ensures the script survives page navigations
    pub async fn add_script_on_new_document(&self, script: &str) -> Result<()> {
        let page = self.get_active_page().await?;

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
        let page = self.get_active_page().await?;
        page.bring_to_front()
            .await
            .map_err(|e| anyhow!("Failed to bring browser to front: {}", e))?;

        tracing::debug!("Browser window brought to front");
        Ok(())
    }

    /// Close the browser
    pub async fn close(&self) -> Result<()> {
        let mut pages = self.pages.lock().await;
        let mut browser_guard = self.browser.lock().await;

        // Close all pages first
        for page in pages.drain(..) {
            let _ = page.close().await;
        }

        // Then close browser
        if let Some(mut browser) = browser_guard.take() {
            let _ = browser.close().await;
        }

        *self.active_tab.lock().await = 0;
        tracing::info!("Browser closed");
        Ok(())
    }

    /// Get the underlying page for advanced operations
    pub async fn page(&self) -> Option<Page> {
        self.get_active_page().await.ok()
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
