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
use chromiumoxide::cdp::browser_protocol::page::{AddScriptToEvaluateOnNewDocumentParams, CaptureScreenshotFormat};
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
        config = config
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--disable-infobars")
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
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Get the EXISTING default page - don't create a new one!
        // This is the key fix - Chrome already has one window/tab open
        let pages = browser.pages().await.map_err(|e| anyhow!("Failed to get pages: {}", e))?;

        let page = pages.into_iter().next()
            .ok_or_else(|| anyhow!("No default page found"))?;

        tracing::debug!("Using existing default page");

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

    /// Click element by backend_node_id
    pub async fn click_by_backend_id(&self, backend_id: BackendNodeId) -> Result<()> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        // Get box model to find click coordinates
        let box_params = GetBoxModelParams {
            node_id: None,
            backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
            object_id: None,
        };

        let box_result = page.execute(box_params)
            .await
            .map_err(|e| anyhow!("Failed to get box model for backend_node_id {}: {}", backend_id, e))?;

        let model = box_result.result.model;
        let content = model.content.inner();

        // Calculate center point (content quad is [x1,y1, x2,y2, x3,y3, x4,y4])
        let center_x = (content[0] + content[2] + content[4] + content[6]) / 4.0;
        let center_y = (content[1] + content[3] + content[5] + content[7]) / 4.0;

        // Scroll element into view first
        let scroll_params = ScrollIntoViewIfNeededParams {
            node_id: None,
            backend_node_id: Some(CdpBackendNodeId::new(backend_id)),
            object_id: None,
            rect: None,
        };
        let _ = page.execute(scroll_params).await;

        // Small delay for scroll
        tokio::time::sleep(Duration::from_millis(100)).await;

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
}

impl Default for BrowserManager {
    fn default() -> Self {
        Self::new()
    }
}
