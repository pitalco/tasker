//! Cross-platform screenshot capture using xcap
//!
//! Provides functionality for capturing screen, window, and monitor screenshots.

use image::{DynamicImage, RgbaImage};
use xcap::{Monitor, Window};

/// Screen capture utilities
pub struct ScreenCapture;

impl ScreenCapture {
    /// Capture the entire primary monitor
    pub fn capture_primary_screen() -> anyhow::Result<RgbaImage> {
        let monitors = Monitor::all().map_err(|e| anyhow::anyhow!("Failed to get monitors: {}", e))?;

        let primary = monitors
            .into_iter()
            .find(|m| m.is_primary())
            .ok_or_else(|| anyhow::anyhow!("No primary monitor found"))?;

        let image = primary
            .capture_image()
            .map_err(|e| anyhow::anyhow!("Failed to capture screen: {}", e))?;

        Ok(image)
    }

    /// Capture a specific monitor by index
    pub fn capture_monitor(index: usize) -> anyhow::Result<RgbaImage> {
        let monitors = Monitor::all().map_err(|e| anyhow::anyhow!("Failed to get monitors: {}", e))?;

        let monitor = monitors
            .into_iter()
            .nth(index)
            .ok_or_else(|| anyhow::anyhow!("Monitor index {} not found", index))?;

        let image = monitor
            .capture_image()
            .map_err(|e| anyhow::anyhow!("Failed to capture monitor: {}", e))?;

        Ok(image)
    }

    /// Get information about all monitors
    pub fn list_monitors() -> anyhow::Result<Vec<MonitorInfo>> {
        let monitors = Monitor::all().map_err(|e| anyhow::anyhow!("Failed to get monitors: {}", e))?;

        Ok(monitors
            .into_iter()
            .enumerate()
            .map(|(index, m)| MonitorInfo {
                index,
                name: m.name().to_string(),
                x: m.x(),
                y: m.y(),
                width: m.width(),
                height: m.height(),
                is_primary: m.is_primary(),
                scale_factor: m.scale_factor(),
            })
            .collect())
    }

    /// Get primary monitor dimensions
    pub fn primary_screen_size() -> anyhow::Result<(u32, u32)> {
        let monitors = Monitor::all().map_err(|e| anyhow::anyhow!("Failed to get monitors: {}", e))?;

        let primary = monitors
            .into_iter()
            .find(|m| m.is_primary())
            .ok_or_else(|| anyhow::anyhow!("No primary monitor found"))?;

        Ok((primary.width(), primary.height()))
    }

    /// Capture a specific window by title (partial match)
    pub fn capture_window_by_title(title: &str) -> anyhow::Result<RgbaImage> {
        let windows = Window::all().map_err(|e| anyhow::anyhow!("Failed to get windows: {}", e))?;

        let window = windows
            .into_iter()
            .find(|w| w.title().to_lowercase().contains(&title.to_lowercase()))
            .ok_or_else(|| anyhow::anyhow!("Window with title '{}' not found", title))?;

        let image = window
            .capture_image()
            .map_err(|e| anyhow::anyhow!("Failed to capture window: {}", e))?;

        Ok(image)
    }

    /// List all visible windows
    pub fn list_windows() -> anyhow::Result<Vec<WindowInfo>> {
        let windows = Window::all().map_err(|e| anyhow::anyhow!("Failed to get windows: {}", e))?;

        Ok(windows
            .into_iter()
            .filter(|w| !w.title().is_empty() && !w.is_minimized())
            .map(|w| WindowInfo {
                id: w.id(),
                title: w.title().to_string(),
                app_name: w.app_name().to_string(),
                x: w.x(),
                y: w.y(),
                width: w.width(),
                height: w.height(),
                is_minimized: w.is_minimized(),
            })
            .collect())
    }

    /// Convert an RgbaImage to base64 PNG
    pub fn image_to_base64(image: &RgbaImage) -> anyhow::Result<String> {
        use base64::Engine;
        use image::ImageEncoder;
        use std::io::Cursor;

        let mut buffer = Cursor::new(Vec::new());
        let encoder = image::codecs::png::PngEncoder::new(&mut buffer);
        encoder
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| anyhow::anyhow!("Failed to encode PNG: {}", e))?;

        let base64_string = base64::engine::general_purpose::STANDARD.encode(buffer.into_inner());
        Ok(base64_string)
    }

    /// Convert an RgbaImage to DynamicImage for processing
    pub fn to_dynamic_image(image: RgbaImage) -> DynamicImage {
        DynamicImage::ImageRgba8(image)
    }
}

/// Information about a monitor
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
    pub scale_factor: f32,
}

/// Information about a window
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: u32,
    pub title: String,
    pub app_name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_minimized: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_monitors() {
        // This test may fail in CI environments without displays
        if let Ok(monitors) = ScreenCapture::list_monitors() {
            assert!(!monitors.is_empty(), "Should have at least one monitor");
            assert!(monitors.iter().any(|m| m.is_primary), "Should have a primary monitor");
        }
    }
}
