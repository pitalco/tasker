use anyhow::{anyhow, Result};
use chrono::Utc;
use chromiumoxide::cdp::js_protocol::runtime::EventBindingCalled;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::browser::BrowserManager;
use crate::models::{
    ActionType, ActionHints, BrowserAction, Coordinates, ElementSelector, RecordedAction,
    RecordedWorkflow, RecordingSession, SelectorStrategy, Viewport, Workflow, WorkflowMetadata,
    WorkflowStep,
};

/// Event captured from browser JavaScript
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CapturedEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    x: Option<i32>,
    #[serde(default)]
    y: Option<i32>,
    #[serde(default)]
    selector: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    timestamp: i64,
    // Additional hints for AI
    #[serde(default)]
    tag_name: Option<String>,
    #[serde(default)]
    aria_label: Option<String>,
    #[serde(default)]
    placeholder: Option<String>,
    #[serde(default)]
    input_type: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    href: Option<String>,
}

/// JavaScript for injecting recording listeners into the page
/// SIMPLE version - capture what matters for replay
const RECORDING_SCRIPT: &str = r#"
(() => {
    if (window.__taskerRecording) return true;
    window.__taskerRecording = true;

    // Track what we've sent to avoid duplicates
    let lastSentValue = new Map();

    // Send event to Rust via CDP binding
    function send(event) {
        try {
            if (typeof __taskerCaptureEvent === 'function') {
                __taskerCaptureEvent(JSON.stringify(event));
                console.log('[Tasker]', event.type, event);
            }
        } catch (e) {
            console.error('[Tasker] Error:', e);
        }
    }

    // Simple selector
    function getSelector(el) {
        if (!el) return null;
        if (el.id) return '#' + el.id;
        if (el.name) return `[name="${el.name}"]`;
        if (el.className && typeof el.className === 'string' && el.className.trim()) {
            return el.tagName.toLowerCase() + '.' + el.className.trim().split(/\s+/)[0];
        }
        return el.tagName.toLowerCase();
    }

    // Get text from element
    function getText(el) {
        if (!el) return '';
        return (el.innerText || el.textContent || el.value || '').slice(0, 100).trim();
    }

    // Send input value if changed
    function sendInputValue(el, reason) {
        if (!el || el.type === 'password') return;
        if (el.tagName !== 'INPUT' && el.tagName !== 'TEXTAREA') return;

        const selector = getSelector(el);
        const value = el.value;

        // Only send if value changed from last sent
        if (lastSentValue.get(selector) === value) return;
        lastSentValue.set(selector, value);

        send({
            type: 'type',
            value: value,
            selector: selector,
            placeholder: el.placeholder || null,
            tag_name: el.tagName.toLowerCase(),
            timestamp: Date.now()
        });
        console.log('[Tasker] Input captured (' + reason + '):', value);
    }

    // CLICK - capture immediately, but first flush any pending input
    document.addEventListener('click', (e) => {
        // If clicking away from an input, capture its value first
        const active = document.activeElement;
        if (active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA')) {
            sendInputValue(active, 'before-click');
        }

        const el = e.target;
        send({
            type: 'click',
            x: Math.round(e.pageX),
            y: Math.round(e.pageY),
            selector: getSelector(el),
            text: getText(el),
            tag_name: el.tagName.toLowerCase(),
            timestamp: Date.now()
        });
    }, true);

    // INPUT - track changes but debounce
    let inputTimer = null;
    document.addEventListener('input', (e) => {
        const el = e.target;
        if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') {
            clearTimeout(inputTimer);
            inputTimer = setTimeout(() => sendInputValue(el, 'debounce'), 800);
        }
    }, true);

    // BLUR - capture final value when leaving field
    document.addEventListener('blur', (e) => {
        clearTimeout(inputTimer);
        sendInputValue(e.target, 'blur');
    }, true);

    // KEYDOWN - handle Enter, Tab (these often trigger actions)
    document.addEventListener('keydown', (e) => {
        const el = e.target;

        // Enter or Tab in an input - capture value FIRST, then the keypress
        if (e.key === 'Enter' || e.key === 'Tab') {
            if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') {
                clearTimeout(inputTimer);
                sendInputValue(el, 'before-' + e.key.toLowerCase());
            }

            send({
                type: 'keypress',
                value: e.key,
                selector: getSelector(el),
                tag_name: el.tagName ? el.tagName.toLowerCase() : null,
                timestamp: Date.now()
            });
        }
    }, true);

    // SELECT - capture changes
    document.addEventListener('change', (e) => {
        const el = e.target;
        if (el.tagName === 'SELECT') {
            const opt = el.options[el.selectedIndex];
            send({
                type: 'select',
                value: el.value,
                text: opt ? opt.text : el.value,
                selector: getSelector(el),
                tag_name: 'select',
                timestamp: Date.now()
            });
        }
    }, true);

    // BEFORE UNLOAD - last chance to capture any pending input
    window.addEventListener('beforeunload', () => {
        const active = document.activeElement;
        if (active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA')) {
            sendInputValue(active, 'beforeunload');
        }
    });

    console.log('[Tasker] Recording ready');
    return true;
})()
"#;

/// Browser recorder that captures user actions
pub struct BrowserRecorder {
    /// The browser manager - public for integration tests
    pub browser: Arc<BrowserManager>,
    session: Arc<Mutex<Option<RecordingSession>>>,
    step_order: Arc<Mutex<i32>>,
    step_sender: broadcast::Sender<WorkflowStep>,
    cancel_sender: broadcast::Sender<()>,
    viewport: Viewport,
    /// Rolling screenshot state - previous action's "after" becomes current action's "before"
    last_screenshot: Arc<Mutex<Option<String>>>,
}

impl BrowserRecorder {
    pub fn new() -> Self {
        let (step_tx, _) = broadcast::channel(256);
        let (cancel_tx, _) = broadcast::channel(1);

        Self {
            browser: Arc::new(BrowserManager::new()),
            session: Arc::new(Mutex::new(None)),
            step_order: Arc::new(Mutex::new(0)),
            step_sender: step_tx,
            cancel_sender: cancel_tx,
            viewport: Viewport {
                width: 1280,
                height: 720,
            },
            last_screenshot: Arc::new(Mutex::new(None)),
        }
    }

    /// Start a new recording session (uses incognito by default for clean sessions)
    pub async fn start(
        &self,
        start_url: &str,
        headless: bool,
        viewport: Option<Viewport>,
    ) -> Result<RecordingSession> {
        self.start_with_options(start_url, headless, viewport, true).await
    }

    /// Start a recording session with full options
    pub async fn start_with_options(
        &self,
        start_url: &str,
        headless: bool,
        viewport: Option<Viewport>,
        incognito: bool,
    ) -> Result<RecordingSession> {
        let viewport = viewport.unwrap_or_else(|| self.viewport.clone());

        // Create session
        let mut session = RecordingSession::new(start_url.to_string());
        session.start();
        let session_id = session.id.clone();

        // Store session
        *self.session.lock().await = Some(session.clone());

        // Launch browser to about:blank first (incognito by default for clean sessions)
        // This allows us to set up recording BEFORE the target page loads
        self.browser
            .launch_with_options(start_url, headless, Some(viewport), incognito)
            .await?;

        // Set up CDP binding for instant event capture (NO POLLING!)
        let event_stream = self.browser
            .setup_event_binding("__taskerCaptureEvent")
            .await?;

        // Register script to run on EVERY new document (survives navigations!)
        // This will auto-inject when we navigate to the target URL
        self.browser.add_script_on_new_document(RECORDING_SCRIPT).await?;

        // Start event listener in background
        self.spawn_event_listener(event_stream);

        // NOW navigate to target URL - recording script will auto-inject as page loads
        self.browser.navigate(start_url).await?;

        tracing::info!("Recording started: {} (script injected before page load)", session_id);

        Ok(session)
    }

    /// Spawn background task to listen for CDP binding events (INSTANT, no polling!)
    fn spawn_event_listener(&self, mut event_stream: chromiumoxide::listeners::EventStream<EventBindingCalled>) {
        let browser = Arc::clone(&self.browser);
        let session = Arc::clone(&self.session);
        let step_order = Arc::clone(&self.step_order);
        let step_sender = self.step_sender.clone();
        let mut cancel_rx = self.cancel_sender.subscribe();
        let last_screenshot = Arc::clone(&self.last_screenshot);

        tokio::spawn(async move {
            let mut last_url = String::new();
            let mut url_check_count: u32 = 0;

            // Take initial screenshot before any user actions
            // This becomes the "before" screenshot for the first action
            match browser.screenshot().await {
                Ok(initial_screenshot) => {
                    *last_screenshot.lock().await = Some(initial_screenshot);
                    tracing::debug!("Captured initial page screenshot for recording");
                }
                Err(e) => {
                    tracing::warn!("Failed to capture initial screenshot: {}", e);
                }
            }

            loop {
                tokio::select! {
                    // Check for cancellation
                    _ = cancel_rx.recv() => {
                        tracing::info!("Recording event listener cancelled");
                        break;
                    }
                    // Wait for CDP binding events (INSTANT when JS calls __taskerCaptureEvent)
                    maybe_event = event_stream.next() => {
                        match maybe_event {
                            Some(binding_event) => {
                                // Only process events from our binding
                                if binding_event.name != "__taskerCaptureEvent" {
                                    continue;
                                }

                                // Check if session is still recording
                                let session_guard = session.lock().await;
                                let is_recording = session_guard
                                    .as_ref()
                                    .map(|s| s.status == "recording")
                                    .unwrap_or(false);
                                drop(session_guard);

                                if !is_recording {
                                    continue;
                                }

                                // Parse the event payload (JSON string from JavaScript)
                                if let Ok(event) = serde_json::from_str::<CapturedEvent>(&binding_event.payload) {
                                    if let Some(mut step) = create_step_from_event(&step_order, &event).await {
                                        // Get "before" screenshot from rolling state
                                        let screenshot_before = last_screenshot.lock().await.clone();
                                        step.screenshot_before = screenshot_before;

                                        // Wait a moment for DOM to settle after the action
                                        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

                                        // Capture "after" screenshot
                                        match browser.screenshot().await {
                                            Ok(after_screenshot) => {
                                                step.screenshot_after = Some(after_screenshot.clone());
                                                // Store as next action's "before"
                                                *last_screenshot.lock().await = Some(after_screenshot);
                                            }
                                            Err(e) => {
                                                tracing::warn!("Failed to capture after screenshot: {}", e);
                                            }
                                        }

                                        // Add to session
                                        let mut session_guard = session.lock().await;
                                        if let Some(ref mut sess) = *session_guard {
                                            sess.steps.push(step.clone());
                                        }
                                        drop(session_guard);

                                        // Broadcast step
                                        let _ = step_sender.send(step);
                                    }
                                }

                                // Track URL changes for logging (script auto-injects via addScriptToEvaluateOnNewDocument)
                                url_check_count += 1;
                                if url_check_count.is_multiple_of(10) {
                                    if let Ok(current_url) = browser.current_url().await {
                                        if current_url != last_url && !last_url.is_empty() {
                                            tracing::debug!("Navigation detected: {} -> {}", last_url, current_url);
                                        }
                                        last_url = current_url;
                                    }
                                }
                            }
                            None => {
                                // Event stream ended (page closed?)
                                tracing::debug!("CDP event stream ended");
                                break;
                            }
                        }
                    }
                }
            }

            tracing::info!("Recording event listener stopped");
        });
    }

    /// Pause recording
    pub async fn pause(&self) -> Result<()> {
        let mut session_guard = self.session.lock().await;
        if let Some(ref mut session) = *session_guard {
            session.pause();
            self.browser
                .evaluate("window.__taskerPaused = true; true")
                .await?;
            tracing::info!("Recording paused");
        }
        Ok(())
    }

    /// Resume recording
    pub async fn resume(&self) -> Result<()> {
        let mut session_guard = self.session.lock().await;
        if let Some(ref mut session) = *session_guard {
            session.resume();
            self.browser
                .evaluate("window.__taskerPaused = false; true")
                .await?;
            tracing::info!("Recording resumed");
        }
        Ok(())
    }

    /// Stop recording and return the workflow
    pub async fn stop(&self) -> Result<Workflow> {
        // Signal event listener to stop
        let _ = self.cancel_sender.send(());

        let mut session_guard = self.session.lock().await;
        let session = session_guard
            .take()
            .ok_or_else(|| anyhow!("No active recording session"))?;
        drop(session_guard); // Release lock early

        // Consolidate consecutive type events for the same selector
        let consolidated_steps = consolidate_type_events(session.steps);
        let step_count = consolidated_steps.len();

        // Create workflow from session
        let mut workflow = Workflow::new(
            format!("Recording {}", Utc::now().format("%Y-%m-%d %H:%M")),
            session.start_url.clone(),
        );
        workflow.steps = consolidated_steps;
        workflow.metadata = WorkflowMetadata {
            recording_source: "recorded".to_string(),
            browser_viewport: Some(self.viewport.clone()),
            user_agent: None,
            tags: vec![],
            start_url: None,
            llm_provider: None,
        };

        tracing::info!("Recording stopped, workflow created: {} ({} steps)", workflow.id, step_count);

        // Close browser in background to avoid lag on stop
        let browser = Arc::clone(&self.browser);
        tokio::spawn(async move {
            if let Err(e) = browser.close().await {
                tracing::warn!("Background browser close failed: {}", e);
            }
        });

        Ok(workflow)
    }

    /// Stop recording and return as tool-based RecordedWorkflow format
    /// This format is optimized for AI hints when running the workflow
    pub async fn stop_as_recorded(&self) -> Result<RecordedWorkflow> {
        // Signal event listener to stop
        let _ = self.cancel_sender.send(());

        let mut session_guard = self.session.lock().await;
        let session = session_guard
            .take()
            .ok_or_else(|| anyhow!("No active recording session"))?;
        drop(session_guard); // Release lock early

        // Consolidate consecutive type events for the same selector
        let consolidated_steps = consolidate_type_events(session.steps);

        // Convert steps to RecordedAction format
        let actions: Vec<RecordedAction> = consolidated_steps
            .iter()
            .map(convert_step_to_recorded_action)
            .collect();

        let recorded = RecordedWorkflow {
            id: Uuid::new_v4().to_string(),
            name: format!("Recording {}", Utc::now().format("%Y-%m-%d %H:%M")),
            description: None,
            start_url: session.start_url.clone(),
            actions,
            created_at: Utc::now(),
        };

        tracing::info!(
            "Recording stopped, recorded workflow created: {} with {} actions",
            recorded.id,
            recorded.actions.len()
        );

        // Close browser in background to avoid lag on stop
        let browser = Arc::clone(&self.browser);
        tokio::spawn(async move {
            if let Err(e) = browser.close().await {
                tracing::warn!("Background browser close failed: {}", e);
            }
        });

        Ok(recorded)
    }

    /// Cancel recording without saving
    pub async fn cancel(&self) -> Result<()> {
        // Signal poller to stop
        let _ = self.cancel_sender.send(());

        let mut session_guard = self.session.lock().await;
        if let Some(ref mut session) = *session_guard {
            session.fail("Recording cancelled by user".to_string());
        }
        *session_guard = None;

        // Close browser
        self.browser.close().await?;

        tracing::info!("Recording cancelled");

        Ok(())
    }

    /// Get the current session
    pub async fn session(&self) -> Option<RecordingSession> {
        self.session.lock().await.clone()
    }

    /// Get step count
    pub async fn step_count(&self) -> usize {
        self.session
            .lock()
            .await
            .as_ref()
            .map(|s| s.steps.len())
            .unwrap_or(0)
    }

    /// Subscribe to step events
    pub fn subscribe_steps(&self) -> broadcast::Receiver<WorkflowStep> {
        self.step_sender.subscribe()
    }
}

impl Default for BrowserRecorder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a workflow step from a captured event
async fn create_step_from_event(
    step_order: &Mutex<i32>,
    event: &CapturedEvent,
) -> Option<WorkflowStep> {
    let mut order = step_order.lock().await;
    *order += 1;
    let order_val = *order;

    let (action, name) = match event.event_type.as_str() {
        "click" => {
            let selector = event.selector.as_ref().map(|s| ElementSelector {
                strategy: SelectorStrategy::Css,
                value: s.clone(),
                fallback_selectors: vec![],
            });
            let coords = event.x.zip(event.y).map(|(x, y)| Coordinates { x, y });
            let text = event.text.clone().unwrap_or_default();
            let name = if text.is_empty() {
                format!("Click {}", event.tag_name.as_deref().unwrap_or("element"))
            } else {
                format!("Click {}", truncate_str(&text, 30))
            };

            // Store additional hints in options
            let mut options = std::collections::HashMap::new();
            if let Some(tag) = &event.tag_name {
                options.insert("tag_name".to_string(), serde_json::Value::String(tag.clone()));
            }
            if let Some(aria) = &event.aria_label {
                options.insert("aria_label".to_string(), serde_json::Value::String(aria.clone()));
            }
            if let Some(role) = &event.role {
                options.insert("role".to_string(), serde_json::Value::String(role.clone()));
            }
            if let Some(href) = &event.href {
                options.insert("href".to_string(), serde_json::Value::String(href.clone()));
            }
            if !text.is_empty() {
                options.insert("element_text".to_string(), serde_json::Value::String(text));
            }

            (
                BrowserAction {
                    action_type: ActionType::Click,
                    selector,
                    value: None,
                    url: None,
                    coordinates: coords,
                    options,
                    clear_first: None,
                },
                name,
            )
        }
        "type" => {
            let selector = event.selector.as_ref().map(|s| ElementSelector {
                strategy: SelectorStrategy::Css,
                value: s.clone(),
                fallback_selectors: vec![],
            });
            let value = event.value.clone().unwrap_or_default();
            let placeholder = event.placeholder.clone().unwrap_or_default();
            let name = if !placeholder.is_empty() {
                format!("Type into '{}'", truncate_str(&placeholder, 20))
            } else {
                format!("Type into {}", event.tag_name.as_deref().unwrap_or("input"))
            };

            // Store additional hints in options
            let mut options = std::collections::HashMap::new();
            if let Some(ph) = &event.placeholder {
                options.insert("placeholder".to_string(), serde_json::Value::String(ph.clone()));
            }
            if let Some(input_type) = &event.input_type {
                options.insert("input_type".to_string(), serde_json::Value::String(input_type.clone()));
            }

            (
                BrowserAction {
                    action_type: ActionType::Type,
                    selector,
                    value: Some(value),
                    url: None,
                    coordinates: None,
                    options,
                    clear_first: None,
                },
                name,
            )
        }
        "select" => {
            let selector = event.selector.as_ref().map(|s| ElementSelector {
                strategy: SelectorStrategy::Css,
                value: s.clone(),
                fallback_selectors: vec![],
            });
            let value = event.value.clone().unwrap_or_default();
            let text = event.text.clone().unwrap_or_else(|| value.clone());
            let name = format!("Select '{}'", truncate_str(&text, 30));

            let mut options = std::collections::HashMap::new();
            if !text.is_empty() && text != value {
                options.insert("option_text".to_string(), serde_json::Value::String(text));
            }

            (
                BrowserAction {
                    action_type: ActionType::Select,
                    selector,
                    value: Some(value),
                    url: None,
                    coordinates: None,
                    options,
                    clear_first: None,
                },
                name,
            )
        }
        "keypress" => {
            let key = event.value.clone().unwrap_or_default();
            let name = format!("Press {}", key);

            // Include target element selector if available
            let selector = event.selector.as_ref().map(|s| ElementSelector {
                strategy: SelectorStrategy::Css,
                value: s.clone(),
                fallback_selectors: vec![],
            });

            let mut options = std::collections::HashMap::new();
            options.insert("key".to_string(), serde_json::Value::String(key.clone()));
            if let Some(tag) = &event.tag_name {
                options.insert("tag_name".to_string(), serde_json::Value::String(tag.clone()));
            }

            (
                BrowserAction {
                    action_type: ActionType::Custom,
                    selector,
                    value: Some(key),
                    url: None,
                    coordinates: None,
                    options,
                    clear_first: None,
                },
                name,
            )
        }
        _ => return None,
    };

    Some(WorkflowStep {
        id: Uuid::new_v4().to_string(),
        order: order_val,
        name,
        description: None,
        action,
        screenshot_before: None,
        screenshot_after: None,
        screenshot_path: None,
        dom_snapshot: None,
        wait_after_ms: 500,
        retry_count: 3,
        timeout_ms: 30000,
    })
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

/// Consolidate consecutive type events for the same selector into a single step.
/// This handles the case where typing "hello world" with pauses generates multiple events.
/// We keep only the FINAL value for each selector when there are consecutive type events.
fn consolidate_type_events(steps: Vec<WorkflowStep>) -> Vec<WorkflowStep> {
    if steps.is_empty() {
        return steps;
    }

    let mut consolidated: Vec<WorkflowStep> = Vec::with_capacity(steps.len());

    for step in steps {
        // Check if this is a Type action
        if step.action.action_type == ActionType::Type {
            let current_selector = step.action.selector.as_ref().map(|s| &s.value);

            // Check if the last consolidated step is also a Type for the same selector
            if let Some(last) = consolidated.last_mut() {
                if last.action.action_type == ActionType::Type {
                    let last_selector = last.action.selector.as_ref().map(|s| &s.value);

                    // Same selector - update the value instead of adding new step
                    if current_selector == last_selector {
                        // Keep the final typed value (from current step)
                        last.action.value = step.action.value;
                        // Update the name to reflect the final value
                        let value = last.action.value.clone().unwrap_or_default();
                        let placeholder = last.action.options.get("placeholder")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        last.name = if !placeholder.is_empty() {
                            format!("Type into '{}'", truncate_str(placeholder, 20))
                        } else {
                            format!("Type '{}'", truncate_str(&value, 30))
                        };
                        tracing::debug!("Consolidated type event for selector {:?}", current_selector);
                        continue; // Skip adding this step
                    }
                }
            }
        }

        // Not a duplicate type - add normally
        consolidated.push(step);
    }

    // Re-number the order field after consolidation
    for (i, step) in consolidated.iter_mut().enumerate() {
        step.order = (i + 1) as i32;
    }

    consolidated
}

/// Convert a WorkflowStep to a RecordedAction (tool-based format)
fn convert_step_to_recorded_action(step: &WorkflowStep) -> RecordedAction {
    use serde_json::json;

    let timestamp = chrono::Utc::now().timestamp_millis();
    let selector = step.action.selector.as_ref().map(|s| s.value.clone());

    // Extract hints from the options field
    let tag_name = step.action.options.get("tag_name").and_then(|v| v.as_str().map(|s| s.to_string()));
    let aria_label = step.action.options.get("aria_label").and_then(|v| v.as_str().map(|s| s.to_string()));
    let placeholder = step.action.options.get("placeholder").and_then(|v| v.as_str().map(|s| s.to_string()));
    let element_text = step.action.options.get("element_text").and_then(|v| v.as_str().map(|s| s.to_string()));

    match step.action.action_type {
        ActionType::Navigate => RecordedAction {
            order: step.order,
            tool: "go_to_url".to_string(),
            params: json!({ "url": step.action.url.clone().unwrap_or_default() }),
            hints: Some(ActionHints {
                description: Some(step.name.clone()),
                ..Default::default()
            }),
            screenshot: step.screenshot_after.clone(),
            timestamp,
        },
        ActionType::Click => {
            let coords = step.action.coordinates.as_ref().map(|c| (c.x, c.y));
            RecordedAction {
                order: step.order,
                tool: "click_element".to_string(),
                params: json!({}),
                hints: Some(ActionHints {
                    css_selector: selector,
                    text: element_text,
                    tag_name,
                    aria_label,
                    coordinates: coords,
                    description: Some(step.name.clone()),
                    ..Default::default()
                }),
                screenshot: step.screenshot_after.clone(),
                timestamp,
            }
        }
        ActionType::Type => RecordedAction {
            order: step.order,
            tool: "input_text".to_string(),
            params: json!({ "text": step.action.value.clone().unwrap_or_default() }),
            hints: Some(ActionHints {
                css_selector: selector,
                placeholder,
                tag_name,
                description: Some(step.name.clone()),
                ..Default::default()
            }),
            screenshot: step.screenshot_after.clone(),
            timestamp,
        },
        ActionType::Select => RecordedAction {
            order: step.order,
            tool: "select_dropdown_option".to_string(),
            params: json!({ "option": step.action.value.clone().unwrap_or_default() }),
            hints: Some(ActionHints {
                css_selector: selector,
                description: Some(step.name.clone()),
                ..Default::default()
            }),
            screenshot: step.screenshot_after.clone(),
            timestamp,
        },
        ActionType::Scroll => RecordedAction {
            order: step.order,
            tool: "scroll_down".to_string(),
            params: json!({ "amount": 500 }),
            hints: Some(ActionHints {
                description: Some(step.name.clone()),
                ..Default::default()
            }),
            screenshot: step.screenshot_after.clone(),
            timestamp,
        },
        ActionType::Wait => RecordedAction {
            order: step.order,
            tool: "wait".to_string(),
            params: json!({ "seconds": step.wait_after_ms / 1000 }),
            hints: None,
            screenshot: None,
            timestamp,
        },
        ActionType::Screenshot => RecordedAction {
            order: step.order,
            tool: "screenshot".to_string(),
            params: json!({}),
            hints: None,
            screenshot: step.screenshot_after.clone(),
            timestamp,
        },
        ActionType::Custom => {
            // Handle keypress events stored as Custom
            let key = step.action.value.clone().unwrap_or_default();
            RecordedAction {
                order: step.order,
                tool: "send_keys".to_string(),
                params: json!({ "keys": key }),
                hints: Some(ActionHints {
                    description: Some(step.name.clone()),
                    ..Default::default()
                }),
                screenshot: None,
                timestamp,
            }
        }
        _ => {
            // Fallback for Extract, Hover, etc.
            RecordedAction {
                order: step.order,
                tool: "wait".to_string(),
                params: json!({ "seconds": 1 }),
                hints: Some(ActionHints {
                    description: Some(format!("Unsupported action: {}", step.name)),
                    ..Default::default()
                }),
                screenshot: None,
                timestamp,
            }
        }
    }
}
