//! OS-level desktop automation using vision-based approach
//!
//! This module provides cross-platform desktop automation by:
//! 1. Taking screenshots of the screen
//! 2. Overlaying a grid for coordinate reference
//! 3. Using vision-capable LLMs to identify UI elements
//! 4. Simulating input (clicks, typing) at coordinates
//!
//! # Architecture
//!
//! ```text
//! Screenshot (xcap) → Grid Overlay → Vision LLM → Coordinates → Input (enigo)
//! ```
//!
//! # Example
//!
//! ```ignore
//! use tasker_sidecar::desktop::DesktopManager;
//!
//! let mut manager = DesktopManager::new()?;
//!
//! // Take screenshot with grid overlay
//! let (image, description) = manager.capture_with_grid()?;
//!
//! // LLM analyzes image and returns grid cell (e.g., "B5")
//! // Click at the specified cell
//! manager.click_cell("B5")?;
//!
//! // Type some text
//! manager.type_text("Hello, World!")?;
//! ```

pub mod grid;
pub mod input;
pub mod manager;
pub mod screenshot;

pub use grid::{GridOptions, GridOverlay};
pub use input::{InputController, KeyCode, Modifier, MouseButton};
pub use manager::DesktopManager;
pub use screenshot::{MonitorInfo, ScreenCapture, WindowInfo};
