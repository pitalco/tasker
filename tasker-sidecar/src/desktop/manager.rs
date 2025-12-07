use anyhow::{anyhow, Result};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::input::InputSimulator;
use super::platform::{create_provider, AccessibilityProvider};
use super::screenshot::ScreenCapture;
use super::serializer;
use super::types::{OSElementId, OSElementMap, OSExtractionResult, WindowInfo};

/// Desktop automation manager
///
/// Provides high-level interface for OS-level automation, analogous to BrowserManager.
/// Coordinates accessibility provider, input simulation, and screen capture.
pub struct DesktopManager {
    provider: Arc<RwLock<Option<Box<dyn AccessibilityProvider>>>>,
    input: Arc<InputSimulator>,
    capture: Arc<ScreenCapture>,
    current_window: Arc<RwLock<Option<String>>>,
    last_element_map: Arc<RwLock<OSElementMap>>,
}

impl DesktopManager {
    /// Create a new desktop manager
    pub fn new() -> Result<Self> {
        let input = InputSimulator::new()?;
        let capture = ScreenCapture::new();

        // Try to create accessibility provider (may fail on unsupported platforms)
        let provider = create_provider().ok();

        Ok(Self {
            provider: Arc::new(RwLock::new(provider)),
            input: Arc::new(input),
            capture: Arc::new(capture),
            current_window: Arc::new(RwLock::new(None)),
            last_element_map: Arc::new(RwLock::new(OSElementMap::new())),
        })
    }

    /// Check if accessibility features are available
    pub async fn is_accessibility_available(&self) -> bool {
        self.provider.read().await.is_some()
    }

    /// Get all visible windows
    pub async fn get_windows(&self) -> Result<Vec<WindowInfo>> {
        let provider = self.provider.read().await;
        let provider = provider
            .as_ref()
            .ok_or_else(|| anyhow!("Accessibility provider not available"))?;
        provider.get_windows().await
    }

    /// Get the currently focused window
    pub async fn get_focused_window(&self) -> Result<Option<WindowInfo>> {
        let provider = self.provider.read().await;
        let provider = provider
            .as_ref()
            .ok_or_else(|| anyhow!("Accessibility provider not available"))?;
        provider.get_focused_window().await
    }

    /// Focus a specific window
    pub async fn focus_window(&self, window_id: &str) -> Result<()> {
        let provider = self.provider.read().await;
        let provider = provider
            .as_ref()
            .ok_or_else(|| anyhow!("Accessibility provider not available"))?;

        provider.focus_window(window_id).await?;
        *self.current_window.write().await = Some(window_id.to_string());
        Ok(())
    }

    /// Focus a window by index (from get_windows list)
    pub async fn focus_window_by_index(&self, index: usize) -> Result<WindowInfo> {
        let windows = self.get_windows().await?;
        let window = windows
            .get(index.saturating_sub(1)) // Convert to 0-based
            .ok_or_else(|| anyhow!("Window index {} out of range (1-{})", index, windows.len()))?;

        self.focus_window(&window.id).await?;
        Ok(window.clone())
    }

    /// Focus a window by title (partial match)
    pub async fn focus_window_by_title(&self, title: &str) -> Result<WindowInfo> {
        let windows = self.get_windows().await?;
        let title_lower = title.to_lowercase();

        let window = windows
            .iter()
            .find(|w| w.title.to_lowercase().contains(&title_lower))
            .ok_or_else(|| anyhow!("No window found matching title: {}", title))?;

        self.focus_window(&window.id).await?;
        Ok(window.clone())
    }

    /// Get indexed interactive elements from the focused window
    ///
    /// Returns OSExtractionResult with:
    /// - element_map: Indexed elements for tool execution
    /// - llm_representation: Human-readable format for LLM
    /// - window: Info about the extracted window
    /// - screenshot_base64: Optional screenshot
    pub async fn get_indexed_elements(&self) -> Result<OSExtractionResult> {
        let provider = self.provider.read().await;
        let provider = provider
            .as_ref()
            .ok_or_else(|| anyhow!("Accessibility provider not available"))?;

        // Get focused window info
        let window = provider
            .get_focused_window()
            .await?
            .ok_or_else(|| anyhow!("No focused window"))?;

        // Extract elements from window
        let elements = provider.get_elements(&window.id).await?;

        // Build element map (assigns indices)
        let element_map = OSElementMap::from_elements(elements);

        // Store for later lookups
        *self.last_element_map.write().await = element_map.clone();

        // Format for LLM
        let llm_representation = serializer::format_for_llm(&element_map, &window);

        // Take screenshot
        let screenshot_base64 = self.capture.capture_window(&window.id).await.ok();

        Ok(OSExtractionResult {
            element_map,
            llm_representation,
            window,
            screenshot_base64,
        })
    }

    /// Get element ID by index from the last extraction
    pub async fn get_element_id_by_index(&self, index: i32) -> Result<OSElementId> {
        let map = self.last_element_map.read().await;
        map.get_id(index)
            .cloned()
            .ok_or_else(|| anyhow!("Element index {} not found", index))
    }

    /// Click an element by its ID
    pub async fn click_element(&self, element_id: &OSElementId) -> Result<()> {
        let provider = self.provider.read().await;
        let provider = provider
            .as_ref()
            .ok_or_else(|| anyhow!("Accessibility provider not available"))?;

        // Try accessibility invoke first
        if provider.invoke_element(element_id).await.is_ok() {
            return Ok(());
        }

        // Fallback to coordinate-based click
        if let Some(element) = provider.get_element(element_id).await? {
            let (cx, cy) = element.bounds.center();
            self.input.click(cx as i32, cy as i32).await
        } else {
            Err(anyhow!("Element not found: {}", element_id))
        }
    }

    /// Click an element by index
    pub async fn click_element_by_index(&self, index: i32) -> Result<()> {
        let element_id = self.get_element_id_by_index(index).await?;
        self.click_element(&element_id).await
    }

    /// Double click an element by index
    pub async fn double_click_element_by_index(&self, index: i32) -> Result<()> {
        let element_id = self.get_element_id_by_index(index).await?;
        let provider = self.provider.read().await;
        let provider = provider
            .as_ref()
            .ok_or_else(|| anyhow!("Accessibility provider not available"))?;

        if let Some(element) = provider.get_element(&element_id).await? {
            let (cx, cy) = element.bounds.center();
            self.input.double_click(cx as i32, cy as i32).await
        } else {
            Err(anyhow!("Element not found"))
        }
    }

    /// Right click an element by index
    pub async fn right_click_element_by_index(&self, index: i32) -> Result<()> {
        let element_id = self.get_element_id_by_index(index).await?;
        let provider = self.provider.read().await;
        let provider = provider
            .as_ref()
            .ok_or_else(|| anyhow!("Accessibility provider not available"))?;

        if let Some(element) = provider.get_element(&element_id).await? {
            let (cx, cy) = element.bounds.center();
            self.input.right_click(cx as i32, cy as i32).await
        } else {
            Err(anyhow!("Element not found"))
        }
    }

    /// Type text into an element by its ID
    pub async fn type_into_element(&self, element_id: &OSElementId, text: &str) -> Result<()> {
        let provider = self.provider.read().await;
        let provider = provider
            .as_ref()
            .ok_or_else(|| anyhow!("Accessibility provider not available"))?;

        // Try accessibility set_value first
        if provider.set_element_value(element_id, text).await.is_ok() {
            return Ok(());
        }

        // Fallback: click to focus, then type
        self.click_element(element_id).await?;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        self.input.type_text(text).await
    }

    /// Type text into an element by index
    pub async fn type_into_element_by_index(&self, index: i32, text: &str) -> Result<()> {
        let element_id = self.get_element_id_by_index(index).await?;
        self.type_into_element(&element_id, text).await
    }

    /// Type text globally (not into a specific element)
    pub async fn type_text(&self, text: &str) -> Result<()> {
        self.input.type_text(text).await
    }

    /// Send keyboard keys (e.g., "Ctrl+C", "Alt+Tab", "Enter")
    pub async fn send_keys(&self, keys: &str) -> Result<()> {
        self.input.send_keys(keys).await
    }

    /// Click at specific screen coordinates
    pub async fn click_at(&self, x: i32, y: i32) -> Result<()> {
        self.input.click(x, y).await
    }

    /// Double click at coordinates
    pub async fn double_click_at(&self, x: i32, y: i32) -> Result<()> {
        self.input.double_click(x, y).await
    }

    /// Right click at coordinates
    pub async fn right_click_at(&self, x: i32, y: i32) -> Result<()> {
        self.input.right_click(x, y).await
    }

    /// Move mouse to coordinates
    pub async fn move_mouse(&self, x: i32, y: i32) -> Result<()> {
        self.input.move_to(x, y).await
    }

    /// Scroll at position
    pub async fn scroll(&self, x: i32, y: i32, delta_x: i32, delta_y: i32) -> Result<()> {
        self.input.scroll(x, y, delta_x, delta_y).await
    }

    /// Drag from one point to another
    pub async fn drag(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> Result<()> {
        self.input.drag(from_x, from_y, to_x, to_y).await
    }

    /// Take a screenshot of the entire screen
    pub async fn screenshot(&self) -> Result<String> {
        self.capture.capture_screen().await
    }

    /// Take a screenshot of a specific window
    pub async fn screenshot_window(&self, window_id: &str) -> Result<String> {
        self.capture.capture_window(window_id).await
    }

    /// Take a screenshot of the focused window
    pub async fn screenshot_focused_window(&self) -> Result<String> {
        let window = self
            .get_focused_window()
            .await?
            .ok_or_else(|| anyhow!("No focused window"))?;
        self.capture.capture_window(&window.id).await
    }

    /// Launch an application by name or path
    pub async fn launch_app(&self, app: &str, args: &[String]) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", "start", "", app]);
            for arg in args {
                cmd.arg(arg);
            }
            cmd.spawn()
                .map_err(|e| anyhow!("Failed to launch {}: {}", app, e))?;
        }

        #[cfg(target_os = "macos")]
        {
            let mut cmd = Command::new("open");
            cmd.arg("-a").arg(app);
            if !args.is_empty() {
                cmd.arg("--args");
                for arg in args {
                    cmd.arg(arg);
                }
            }
            cmd.spawn()
                .map_err(|e| anyhow!("Failed to launch {}: {}", app, e))?;
        }

        #[cfg(target_os = "linux")]
        {
            let mut cmd = Command::new(app);
            for arg in args {
                cmd.arg(arg);
            }
            cmd.spawn()
                .map_err(|e| anyhow!("Failed to launch {}: {}", app, e))?;
        }

        // Wait a bit for the app to start
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        Ok(())
    }

    /// Close a window (sends Alt+F4 on Windows, Cmd+W on macOS)
    pub async fn close_window(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            self.send_keys("Alt+F4").await?;
        }

        #[cfg(target_os = "macos")]
        {
            self.send_keys("Cmd+W").await?;
        }

        #[cfg(target_os = "linux")]
        {
            self.send_keys("Alt+F4").await?;
        }

        Ok(())
    }

    /// Minimize the focused window
    pub async fn minimize_window(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            self.send_keys("Win+Down").await?;
        }

        #[cfg(target_os = "macos")]
        {
            self.send_keys("Cmd+M").await?;
        }

        #[cfg(target_os = "linux")]
        {
            self.send_keys("Super+H").await?;
        }

        Ok(())
    }

    /// Maximize the focused window
    pub async fn maximize_window(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            self.send_keys("Win+Up").await?;
        }

        #[cfg(target_os = "macos")]
        {
            // macOS doesn't have a direct maximize shortcut
            // Would need to use accessibility to click the green button
            return Err(anyhow!("Maximize not directly supported on macOS"));
        }

        #[cfg(target_os = "linux")]
        {
            self.send_keys("Super+Up").await?;
        }

        Ok(())
    }
}

// Implement Clone manually since we use Arc internally
impl Clone for DesktopManager {
    fn clone(&self) -> Self {
        Self {
            provider: Arc::clone(&self.provider),
            input: Arc::clone(&self.input),
            capture: Arc::clone(&self.capture),
            current_window: Arc::clone(&self.current_window),
            last_element_map: Arc::clone(&self.last_element_map),
        }
    }
}
