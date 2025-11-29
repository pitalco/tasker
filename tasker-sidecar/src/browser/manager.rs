use anyhow::{anyhow, Result};
use base64::Engine;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::cdp::js_protocol::runtime::{AddBindingParams, EventBindingCalled};
use chromiumoxide::listeners::EventStream;
use chromiumoxide::Page;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;

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
    pub async fn launch_with_options(
        &self,
        url: &str,
        headless: bool,
        viewport: Option<Viewport>,
        incognito: bool,
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

        // NOTE: Do NOT use --incognito flag! It creates TWO windows.
        // Instead, use CDP's browser context API below.

        // Disable automation detection flags for cleaner sessions
        // Also disable default apps and extensions to prevent extra windows
        config = config
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--disable-infobars")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-default-apps")
            .arg("--disable-extensions");

        let config = config.build().map_err(|e| anyhow!("Failed to build browser config: {}", e))?;

        // Wrap browser launch with 30-second timeout to prevent indefinite hangs
        let (mut browser, mut handler) = timeout(
            Duration::from_secs(30),
            Browser::launch(config)
        )
        .await
        .map_err(|_| anyhow!("Browser launch timeout (30s) - Chrome may not be installed or is unresponsive"))?
        .map_err(|e| anyhow!("Failed to launch browser: {}", e))?;

        // Spawn handler task to process browser events (filter noisy errors)
        tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                tracing::trace!("Browser event: {:?}", event);
            }
        });

        // Minimal delay for Chrome to initialize (reduced from 100ms)
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Get default pages BEFORE creating incognito context
        // We'll close them AFTER creating our target page
        let default_pages = browser.pages().await.map_err(|e| anyhow!("Failed to get pages: {}", e))?;
        tracing::debug!("Found {} default page(s) to close later", default_pages.len());

        // Use CDP's incognito browser context API (NOT the --incognito flag!)
        // This creates an isolated context without opening extra windows
        if incognito {
            tracing::info!("Creating incognito browser context via CDP");
            browser
                .start_incognito_context()
                .await
                .map_err(|e| anyhow!("Failed to start incognito context: {}", e))?;
        }

        // Create page within the context (incognito or default) and navigate
        let page = browser
            .new_page(url)
            .await
            .map_err(|e| anyhow!("Failed to create page: {}", e))?;

        // NOW close the default pages (after we have our target page)
        // This prevents having multiple windows visible
        for default_page in default_pages {
            if let Err(e) = default_page.close().await {
                tracing::warn!("Failed to close default page: {}", e);
            }
        }

        // Set viewport via emulation
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

        // No sleep needed - viewport is applied synchronously
        // Store browser and page
        *self.browser.lock().await = Some(browser);
        *self.page.lock().await = Some(page);

        tracing::info!("Browser launched{} and navigated to {}",
            if incognito { " (incognito context)" } else { "" }, url);
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

    /// Click on an element
    pub async fn click(&self, selector: &str) -> Result<()> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        let element = page
            .find_element(selector)
            .await
            .map_err(|e| anyhow!("Failed to find element '{}': {}", selector, e))?;

        element
            .click()
            .await
            .map_err(|e| anyhow!("Failed to click element '{}': {}", selector, e))?;

        Ok(())
    }

    /// Type text into an element
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<()> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        let element = page
            .find_element(selector)
            .await
            .map_err(|e| anyhow!("Failed to find element '{}': {}", selector, e))?;

        element
            .click()
            .await
            .map_err(|e| anyhow!("Failed to focus element '{}': {}", selector, e))?;

        element
            .type_str(text)
            .await
            .map_err(|e| anyhow!("Failed to type into element '{}': {}", selector, e))?;

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

    /// Hover over an element
    pub async fn hover(&self, selector: &str) -> Result<()> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        let element = page
            .find_element(selector)
            .await
            .map_err(|e| anyhow!("Failed to find element '{}': {}", selector, e))?;

        element
            .hover()
            .await
            .map_err(|e| anyhow!("Failed to hover over element '{}': {}", selector, e))?;

        Ok(())
    }

    /// Select an option from a dropdown
    pub async fn select(&self, selector: &str, value: &str) -> Result<()> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        // Use JavaScript to select the option
        let script = format!(
            r#"
            const select = document.querySelector('{}');
            if (select) {{
                select.value = '{}';
                select.dispatchEvent(new Event('change', {{ bubbles: true }}));
            }}
            "#,
            selector.replace('\'', "\\'"),
            value.replace('\'', "\\'")
        );

        page.evaluate(script)
            .await
            .map_err(|e| anyhow!("Failed to select option: {}", e))?;

        Ok(())
    }

    /// Wait for a duration
    pub async fn wait(&self, duration_ms: u64) -> Result<()> {
        tokio::time::sleep(tokio::time::Duration::from_millis(duration_ms)).await;
        Ok(())
    }

    /// Wait for an element to appear
    pub async fn wait_for_element(&self, selector: &str, timeout_ms: u64) -> Result<()> {
        let page_guard = self.page.lock().await;
        let page = page_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No page available"))?;

        let timeout = std::time::Duration::from_millis(timeout_ms);
        let start = std::time::Instant::now();

        loop {
            if page.find_element(selector).await.is_ok() {
                return Ok(());
            }

            if start.elapsed() > timeout {
                return Err(anyhow!(
                    "Timeout waiting for element '{}' after {}ms",
                    selector,
                    timeout_ms
                ));
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
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
