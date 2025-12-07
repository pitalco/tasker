//! Desktop automation module
//!
//! Provides OS-level automation capabilities analogous to browser automation.
//! Uses native accessibility APIs to interact with UI elements.
//!
//! ## Architecture
//!
//! The module mirrors the browser automation architecture:
//! - `DesktopManager` - High-level interface (like BrowserManager)
//! - `OSElement` / `OSElementMap` - Element types (like SimplifiedElement / SelectorMap)
//! - `AccessibilityProvider` - Platform abstraction for accessibility APIs
//!
//! ## Supported Platforms
//!
//! - **Windows**: UI Automation API
//! - **macOS**: Accessibility API (AX) - *stub implementation*
//! - **Linux**: AT-SPI2 - *stub implementation*
//!
//! ## Example
//!
//! ```rust,ignore
//! use tasker_sidecar::desktop::DesktopManager;
//!
//! let manager = DesktopManager::new()?;
//!
//! // Get indexed elements from focused window
//! let result = manager.get_indexed_elements().await?;
//! println!("Elements: {}", result.llm_representation);
//!
//! // Click element by index
//! manager.click_element_by_index(1).await?;
//!
//! // Type into element
//! manager.type_into_element_by_index(2, "Hello").await?;
//!
//! // Send keyboard shortcut
//! manager.send_keys("Ctrl+S").await?;
//! ```

pub mod input;
pub mod manager;
pub mod platform;
pub mod screenshot;
pub mod serializer;
pub mod types;

// Re-export main types
pub use manager::DesktopManager;
pub use platform::{check_accessibility_permissions, create_provider, AccessibilityProvider};
pub use screenshot::ScreenCapture;
pub use types::{
    AutomationMode, OSElement, OSElementId, OSElementIndex, OSElementMap, OSExtractionResult,
    OSRect, WindowInfo,
};
