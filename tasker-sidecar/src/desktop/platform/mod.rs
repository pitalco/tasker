use anyhow::Result;
use async_trait::async_trait;

use super::types::{OSElement, OSElementId, WindowInfo};

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

/// Platform-specific accessibility provider trait
/// Abstracts OS-specific accessibility APIs behind a common interface
#[async_trait]
pub trait AccessibilityProvider: Send + Sync {
    /// Get list of all visible windows
    async fn get_windows(&self) -> Result<Vec<WindowInfo>>;

    /// Get the currently focused/active window
    async fn get_focused_window(&self) -> Result<Option<WindowInfo>>;

    /// Extract interactive elements from a specific window
    async fn get_elements(&self, window_id: &str) -> Result<Vec<OSElement>>;

    /// Extract interactive elements from the currently focused window
    async fn get_focused_window_elements(&self) -> Result<Vec<OSElement>> {
        let window = self
            .get_focused_window()
            .await?
            .ok_or_else(|| anyhow::anyhow!("No focused window"))?;
        self.get_elements(&window.id).await
    }

    /// Focus a window by its ID
    async fn focus_window(&self, window_id: &str) -> Result<()>;

    /// Get a specific element by its ID
    async fn get_element(&self, element_id: &OSElementId) -> Result<Option<OSElement>>;

    /// Invoke the default action on an element (click, press, etc.)
    async fn invoke_element(&self, element_id: &OSElementId) -> Result<()>;

    /// Set the value of an editable element
    async fn set_element_value(&self, element_id: &OSElementId, value: &str) -> Result<()>;

    /// Expand an expandable element (tree item, menu, etc.)
    async fn expand_element(&self, element_id: &OSElementId) -> Result<()>;

    /// Collapse an expanded element
    async fn collapse_element(&self, element_id: &OSElementId) -> Result<()>;

    /// Scroll an element into view
    async fn scroll_to_element(&self, element_id: &OSElementId) -> Result<()>;

    /// Toggle a toggleable element (checkbox, toggle button)
    async fn toggle_element(&self, element_id: &OSElementId) -> Result<()>;

    /// Select an item in a selection control (list, combo)
    async fn select_element(&self, element_id: &OSElementId) -> Result<()>;

    /// Get the current text selection from an element (if supported)
    async fn get_element_text(&self, element_id: &OSElementId) -> Result<Option<String>>;
}

/// Create the platform-specific accessibility provider
pub fn create_provider() -> Result<Box<dyn AccessibilityProvider>> {
    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(windows::WindowsAccessibility::new()?))
    }

    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacOSAccessibility::new()?))
    }

    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(linux::LinuxAccessibility::new()?))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err(anyhow::anyhow!("Unsupported platform for accessibility"))
    }
}

/// Check if accessibility features are available and enabled
pub fn check_accessibility_permissions() -> Result<bool> {
    #[cfg(target_os = "windows")]
    {
        // Windows UI Automation is always available
        Ok(true)
    }

    #[cfg(target_os = "macos")]
    {
        // macOS requires explicit accessibility permissions
        macos::check_accessibility_permissions()
    }

    #[cfg(target_os = "linux")]
    {
        // Linux AT-SPI2 availability check
        linux::check_accessibility_available()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Ok(false)
    }
}

/// Request accessibility permissions (macOS only, no-op on other platforms)
pub fn request_accessibility_permissions() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        macos::request_accessibility_permissions()
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}
