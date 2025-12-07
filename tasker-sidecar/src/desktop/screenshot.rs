use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use image::ImageFormat;
use std::io::Cursor;
use xcap::{Monitor, Window};

/// Cross-platform screen capture using xcap
pub struct ScreenCapture;

impl ScreenCapture {
    pub fn new() -> Self {
        Self
    }

    /// Capture the entire primary screen
    pub async fn capture_screen(&self) -> Result<String> {
        // Run capture in blocking task since xcap is synchronous
        tokio::task::spawn_blocking(|| {
            let monitors = Monitor::all().map_err(|e| anyhow!("Failed to get monitors: {}", e))?;

            let primary = monitors
                .into_iter()
                .find(|m| m.is_primary())
                .or_else(|| Monitor::all().ok().and_then(|m| m.into_iter().next()))
                .ok_or_else(|| anyhow!("No monitors found"))?;

            let image = primary
                .capture_image()
                .map_err(|e| anyhow!("Failed to capture screen: {}", e))?;

            encode_image_to_base64(&image)
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    /// Capture a specific monitor by index
    pub async fn capture_monitor(&self, index: usize) -> Result<String> {
        tokio::task::spawn_blocking(move || {
            let monitors = Monitor::all().map_err(|e| anyhow!("Failed to get monitors: {}", e))?;

            let monitor = monitors
                .into_iter()
                .nth(index)
                .ok_or_else(|| anyhow!("Monitor index {} not found", index))?;

            let image = monitor
                .capture_image()
                .map_err(|e| anyhow!("Failed to capture monitor: {}", e))?;

            encode_image_to_base64(&image)
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    /// Capture a specific window by its ID
    pub async fn capture_window(&self, window_id: &str) -> Result<String> {
        let window_id = window_id.to_string();
        tokio::task::spawn_blocking(move || {
            let windows = Window::all().map_err(|e| anyhow!("Failed to get windows: {}", e))?;

            let window = windows
                .into_iter()
                .find(|w| w.id().to_string() == window_id)
                .ok_or_else(|| anyhow!("Window '{}' not found", window_id))?;

            let image = window
                .capture_image()
                .map_err(|e| anyhow!("Failed to capture window: {}", e))?;

            encode_image_to_base64(&image)
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    /// Capture a window by its title (partial match)
    pub async fn capture_window_by_title(&self, title: &str) -> Result<String> {
        let title = title.to_lowercase();
        tokio::task::spawn_blocking(move || {
            let windows = Window::all().map_err(|e| anyhow!("Failed to get windows: {}", e))?;

            let window = windows
                .into_iter()
                .find(|w| w.title().to_lowercase().contains(&title))
                .ok_or_else(|| anyhow!("Window with title containing '{}' not found", title))?;

            let image = window
                .capture_image()
                .map_err(|e| anyhow!("Failed to capture window: {}", e))?;

            encode_image_to_base64(&image)
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    /// Capture a specific region of the screen
    pub async fn capture_region(&self, x: i32, y: i32, width: u32, height: u32) -> Result<String> {
        tokio::task::spawn_blocking(move || {
            let monitors = Monitor::all().map_err(|e| anyhow!("Failed to get monitors: {}", e))?;

            // Find the monitor containing this region
            let monitor = monitors
                .into_iter()
                .find(|m| {
                    let mx = m.x();
                    let my = m.y();
                    let mw = m.width() as i32;
                    let mh = m.height() as i32;
                    x >= mx && y >= my && x < mx + mw && y < my + mh
                })
                .or_else(|| Monitor::all().ok().and_then(|m| m.into_iter().next()))
                .ok_or_else(|| anyhow!("No monitor found for region"))?;

            let full_image = monitor
                .capture_image()
                .map_err(|e| anyhow!("Failed to capture screen: {}", e))?;

            // Calculate region relative to monitor
            let rel_x = (x - monitor.x()) as u32;
            let rel_y = (y - monitor.y()) as u32;

            // Crop the region
            let cropped = image::imageops::crop_imm(&full_image, rel_x, rel_y, width, height);
            let cropped_image = cropped.to_image();

            encode_image_to_base64(&cropped_image)
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    /// List all available monitors
    pub async fn list_monitors(&self) -> Result<Vec<MonitorInfo>> {
        tokio::task::spawn_blocking(|| {
            let monitors = Monitor::all().map_err(|e| anyhow!("Failed to get monitors: {}", e))?;

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
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    /// List all visible windows
    pub async fn list_windows(&self) -> Result<Vec<WindowCaptureInfo>> {
        tokio::task::spawn_blocking(|| {
            let windows = Window::all().map_err(|e| anyhow!("Failed to get windows: {}", e))?;

            Ok(windows
                .into_iter()
                .filter(|w| !w.title().is_empty()) // Filter out windows without titles
                .map(|w| WindowCaptureInfo {
                    id: w.id().to_string(),
                    title: w.title().to_string(),
                    app_name: w.app_name().to_string(),
                    x: w.x(),
                    y: w.y(),
                    width: w.width(),
                    height: w.height(),
                    is_minimized: w.is_minimized(),
                    is_maximized: w.is_maximized(),
                })
                .collect())
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }
}

impl Default for ScreenCapture {
    fn default() -> Self {
        Self::new()
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

/// Information about a window for capture
#[derive(Debug, Clone)]
pub struct WindowCaptureInfo {
    pub id: String,
    pub title: String,
    pub app_name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_minimized: bool,
    pub is_maximized: bool,
}

/// Encode an image to base64 PNG
fn encode_image_to_base64(image: &image::RgbaImage) -> Result<String> {
    let mut buffer = Cursor::new(Vec::new());
    image
        .write_to(&mut buffer, ImageFormat::Png)
        .map_err(|e| anyhow!("Failed to encode image: {}", e))?;
    Ok(BASE64.encode(buffer.get_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_monitors() {
        let capture = ScreenCapture::new();
        let monitors = capture.list_monitors().await;
        // Should succeed and return at least one monitor
        assert!(monitors.is_ok());
        let monitors = monitors.unwrap();
        assert!(!monitors.is_empty());
        // At least one should be primary
        assert!(monitors.iter().any(|m| m.is_primary));
    }

    #[tokio::test]
    async fn test_list_windows() {
        let capture = ScreenCapture::new();
        let windows = capture.list_windows().await;
        // Should succeed (may be empty if no windows)
        assert!(windows.is_ok());
    }
}
