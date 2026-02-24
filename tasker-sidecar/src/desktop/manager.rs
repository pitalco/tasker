use anyhow::{anyhow, Result};
use base64::Engine;
use enigo::{
    Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings,
};
use image::codecs::jpeg::JpegEncoder;
use image::{imageops, RgbaImage};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
use xcap::Monitor;

use super::accessibility::{
    format_elements, AccessibilityProvider, DesktopElement, ElementMap,
};
use super::annotator::annotate_screenshot;

/// Maximum screenshot width sent to LLM (to control token usage)
const MAX_SCREENSHOT_WIDTH: u32 = 1280;
/// JPEG quality for screenshot encoding
const JPEG_QUALITY: u8 = 80;

/// Information about the current desktop state, captured each turn
pub struct DesktopState {
    /// Annotated screenshot with numbered markers (base64 JPEG)
    pub screenshot_base64: String,
    /// Raw unannotated screenshot (base64 JPEG) for zoom tool
    pub raw_screenshot_base64: String,
    /// Text list of interactive elements for LLM
    pub elements_text: String,
    /// Title of the currently active window
    pub active_window: String,
    /// Display info string
    pub display_info: String,
    /// Number of interactive elements found
    pub element_count: usize,
}

/// Information about a visible window
#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowInfo {
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Core desktop control manager
pub struct DesktopManager {
    display_width: u32,
    display_height: u32,
    screenshot_width: u32,
    screenshot_height: u32,
    scale_factor: f64,
    enigo: std::sync::Mutex<Enigo>,
    paused: AtomicBool,
    element_map: RwLock<ElementMap>,
}

impl DesktopManager {
    pub fn new() -> Result<Self> {
        let monitors =
            Monitor::all().map_err(|e| anyhow!("Failed to enumerate monitors: {}", e))?;
        let primary = monitors
            .into_iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .or_else(|| Monitor::all().ok().and_then(|m| m.into_iter().next()))
            .ok_or_else(|| anyhow!("No monitor found"))?;

        let display_width = primary.width().unwrap_or(1920);
        let display_height = primary.height().unwrap_or(1080);

        let (screenshot_width, screenshot_height) = if display_width > MAX_SCREENSHOT_WIDTH {
            let ratio = MAX_SCREENSHOT_WIDTH as f64 / display_width as f64;
            (
                MAX_SCREENSHOT_WIDTH,
                (display_height as f64 * ratio) as u32,
            )
        } else {
            (display_width, display_height)
        };

        let scale_factor = screenshot_width as f64 / display_width as f64;

        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow!("Failed to create input simulator: {}", e))?;

        tracing::info!(
            "DesktopManager initialized: {}x{} display, {}x{} screenshots, scale={:.3}",
            display_width,
            display_height,
            screenshot_width,
            screenshot_height,
            scale_factor
        );

        Ok(Self {
            display_width,
            display_height,
            screenshot_width,
            screenshot_height,
            scale_factor,
            enigo: std::sync::Mutex::new(enigo),
            paused: AtomicBool::new(false),
            element_map: RwLock::new(ElementMap::new()),
        })
    }

    pub fn capture_state(&self) -> Result<DesktopState> {
        let raw_img = self.capture_primary_monitor()?;
        let elements = self.extract_elements()?;

        {
            let mut map = self
                .element_map
                .write()
                .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
            *map = ElementMap::from_elements(&elements);
        }

        let resized = resize_to_fit(&raw_img, self.screenshot_width, self.screenshot_height);
        let annotated = annotate_screenshot(&resized, &elements, self.scale_factor);
        let elements_text = format_elements(&elements);
        let active_window = get_active_window_title();
        let screenshot_base64 = encode_jpeg_base64(&annotated)?;
        let raw_screenshot_base64 = encode_jpeg_base64(&resized)?;

        let display_info = format!(
            "{}x{} (scaled from {}x{})",
            self.screenshot_width, self.screenshot_height, self.display_width, self.display_height
        );

        Ok(DesktopState {
            screenshot_base64,
            raw_screenshot_base64,
            elements_text,
            active_window,
            display_info,
            element_count: elements.len(),
        })
    }

    pub fn click_element(&self, index: usize) -> Result<()> {
        let map = self
            .element_map
            .read()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        let elem = map.get(index).ok_or_else(|| {
            anyhow!(
                "Element [{}] not found. Available: 1-{}",
                index,
                map.len()
            )
        })?;

        if !elem.is_enabled {
            return Err(anyhow!("Element [{}] '{}' is disabled", index, elem.name));
        }

        let (cx, cy) = elem.bounds.center();
        drop(map);

        self.click_at_display(cx as i32, cy as i32, "left", false)
    }

    /// Click at coordinates in SCREENSHOT space (scaled to display before clicking)
    pub fn click_at(&self, x: i32, y: i32, button: &str, double_click: bool) -> Result<()> {
        let display_x = (x as f64 / self.scale_factor) as i32;
        let display_y = (y as f64 / self.scale_factor) as i32;
        self.click_at_display(display_x, display_y, button, double_click)
    }

    /// Click at coordinates in DISPLAY space (native resolution)
    fn click_at_display(
        &self,
        x: i32,
        y: i32,
        button: &str,
        double_click: bool,
    ) -> Result<()> {
        let mut enigo = self
            .enigo
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;

        enigo
            .move_mouse(x, y, Coordinate::Abs)
            .map_err(|e| anyhow!("Failed to move mouse: {}", e))?;

        std::thread::sleep(std::time::Duration::from_millis(50));

        let btn = match button {
            "right" => Button::Right,
            "middle" => Button::Middle,
            _ => Button::Left,
        };

        enigo
            .button(btn, Direction::Click)
            .map_err(|e| anyhow!("Failed to click: {}", e))?;

        if double_click {
            std::thread::sleep(std::time::Duration::from_millis(50));
            enigo
                .button(btn, Direction::Click)
                .map_err(|e| anyhow!("Failed to double-click: {}", e))?;
        }

        Ok(())
    }

    pub fn input_text_at_element(&self, index: usize, text: &str) -> Result<()> {
        self.click_element(index)?;
        std::thread::sleep(std::time::Duration::from_millis(100));
        self.type_text(text)
    }

    pub fn type_text(&self, text: &str) -> Result<()> {
        let mut enigo = self
            .enigo
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        enigo
            .text(text)
            .map_err(|e| anyhow!("Failed to type text: {}", e))?;
        Ok(())
    }

    pub fn key_press(&self, keys: &str) -> Result<()> {
        let mut enigo = self
            .enigo
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;

        let parts: Vec<String> = keys.split('+').map(|s| s.trim().to_lowercase()).collect();

        let mut modifiers = Vec::new();
        let mut main_key = None;

        for part in &parts {
            match part.as_str() {
                "ctrl" | "control" => modifiers.push(Key::Control),
                "alt" => modifiers.push(Key::Alt),
                "shift" => modifiers.push(Key::Shift),
                "win" | "super" | "meta" | "cmd" | "command" => modifiers.push(Key::Meta),
                key_str => main_key = Some(parse_key(key_str)?),
            }
        }

        for modifier in &modifiers {
            enigo
                .key(*modifier, Direction::Press)
                .map_err(|e| anyhow!("Failed to press modifier: {}", e))?;
        }

        if let Some(key) = main_key {
            enigo
                .key(key, Direction::Click)
                .map_err(|e| anyhow!("Failed to press key: {}", e))?;
        }

        for modifier in modifiers.iter().rev() {
            enigo
                .key(*modifier, Direction::Release)
                .map_err(|e| anyhow!("Failed to release modifier: {}", e))?;
        }

        Ok(())
    }

    pub fn mouse_move(&self, x: i32, y: i32) -> Result<()> {
        let display_x = (x as f64 / self.scale_factor) as i32;
        let display_y = (y as f64 / self.scale_factor) as i32;

        let mut enigo = self
            .enigo
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        enigo
            .move_mouse(display_x, display_y, Coordinate::Abs)
            .map_err(|e| anyhow!("Failed to move mouse: {}", e))?;
        Ok(())
    }

    pub fn scroll(&self, direction: &str, amount: i32) -> Result<()> {
        let mut enigo = self
            .enigo
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;

        match direction {
            "up" => enigo
                .scroll(amount, enigo::Axis::Vertical)
                .map_err(|e| anyhow!("Failed to scroll: {}", e))?,
            "down" => enigo
                .scroll(-amount, enigo::Axis::Vertical)
                .map_err(|e| anyhow!("Failed to scroll: {}", e))?,
            "left" => enigo
                .scroll(-amount, enigo::Axis::Horizontal)
                .map_err(|e| anyhow!("Failed to scroll: {}", e))?,
            "right" => enigo
                .scroll(amount, enigo::Axis::Horizontal)
                .map_err(|e| anyhow!("Failed to scroll: {}", e))?,
            _ => return Err(anyhow!("Invalid scroll direction: {}", direction)),
        }

        Ok(())
    }

    pub fn drag(&self, start_x: i32, start_y: i32, end_x: i32, end_y: i32) -> Result<()> {
        let sx = (start_x as f64 / self.scale_factor) as i32;
        let sy = (start_y as f64 / self.scale_factor) as i32;
        let ex = (end_x as f64 / self.scale_factor) as i32;
        let ey = (end_y as f64 / self.scale_factor) as i32;

        let mut enigo = self
            .enigo
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;

        enigo
            .move_mouse(sx, sy, Coordinate::Abs)
            .map_err(|e| anyhow!("Failed to move to start: {}", e))?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        enigo
            .button(Button::Left, Direction::Press)
            .map_err(|e| anyhow!("Failed to press for drag: {}", e))?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        enigo
            .move_mouse(ex, ey, Coordinate::Abs)
            .map_err(|e| anyhow!("Failed to move to end: {}", e))?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        enigo
            .button(Button::Left, Direction::Release)
            .map_err(|e| anyhow!("Failed to release for drag: {}", e))?;

        Ok(())
    }

    pub fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        let windows =
            xcap::Window::all().map_err(|e| anyhow!("Failed to enumerate windows: {}", e))?;

        let infos: Vec<WindowInfo> = windows
            .into_iter()
            .filter(|w| {
                let title = w.title().unwrap_or_default();
                let width = w.width().unwrap_or(0);
                let height = w.height().unwrap_or(0);
                !title.is_empty() && width > 0 && height > 0
            })
            .map(|w| WindowInfo {
                title: w.title().unwrap_or_default(),
                x: w.x().unwrap_or(0),
                y: w.y().unwrap_or(0),
                width: w.width().unwrap_or(0),
                height: w.height().unwrap_or(0),
            })
            .collect();

        Ok(infos)
    }

    /// Focus a window by title (substring match)
    /// Uses PowerShell on Windows to avoid windows crate version conflicts
    pub fn focus_window(&self, title: Option<&str>, process_name: Option<&str>) -> Result<()> {
        let search = title
            .or(process_name)
            .ok_or_else(|| anyhow!("Provide title or process_name"))?;

        #[cfg(target_os = "windows")]
        {
            // Use PowerShell to find and activate window - avoids windows crate conflicts
            let ps_script = if title.is_some() {
                format!(
                    r#"Add-Type -AssemblyName Microsoft.VisualBasic; $procs = Get-Process | Where-Object {{$_.MainWindowTitle -like '*{}*'}} | Select-Object -First 1; if ($procs) {{ [Microsoft.VisualBasic.Interaction]::AppActivate($procs.Id) }}"#,
                    search
                )
            } else {
                format!(
                    r#"Add-Type -AssemblyName Microsoft.VisualBasic; $procs = Get-Process -Name '{}' -ErrorAction SilentlyContinue | Select-Object -First 1; if ($procs) {{ [Microsoft.VisualBasic.Interaction]::AppActivate($procs.Id) }}"#,
                    search
                )
            };

            let result = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &ps_script])
                .output();

            match result {
                Ok(output) if output.status.success() => Ok(()),
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(anyhow!(
                        "Failed to focus window '{}': {}",
                        search,
                        stderr.trim()
                    ))
                }
                Err(e) => Err(anyhow!("Failed to run PowerShell: {}", e)),
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            Err(anyhow!(
                "focus_window not implemented on this platform. Looking for: {:?}",
                search
            ))
        }
    }

    pub fn capture_zoom(&self, x: i32, y: i32, width: i32, height: i32) -> Result<String> {
        let raw_img = self.capture_primary_monitor()?;

        let dx = (x as f64 / self.scale_factor) as u32;
        let dy = (y as f64 / self.scale_factor) as u32;
        let dw = (width as f64 / self.scale_factor) as u32;
        let dh = (height as f64 / self.scale_factor) as u32;

        let dx = dx.min(raw_img.width().saturating_sub(1));
        let dy = dy.min(raw_img.height().saturating_sub(1));
        let dw = dw.min(raw_img.width() - dx);
        let dh = dh.min(raw_img.height() - dy);

        if dw == 0 || dh == 0 {
            return Err(anyhow!("Invalid zoom region"));
        }

        let cropped = image::imageops::crop_imm(&raw_img, dx, dy, dw, dh).to_image();
        encode_jpeg_base64(&cropped)
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    fn capture_primary_monitor(&self) -> Result<RgbaImage> {
        let monitors =
            Monitor::all().map_err(|e| anyhow!("Failed to enumerate monitors: {}", e))?;
        let primary = monitors
            .into_iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .or_else(|| Monitor::all().ok().and_then(|m| m.into_iter().next()))
            .ok_or_else(|| anyhow!("No monitor found"))?;

        let img = primary
            .capture_image()
            .map_err(|e| anyhow!("Failed to capture screenshot: {}", e))?;

        Ok(img)
    }

    fn extract_elements(&self) -> Result<Vec<DesktopElement>> {
        #[cfg(target_os = "windows")]
        {
            let provider = super::accessibility::WindowsAccessibility::new()?;
            provider.extract_interactive_elements()
        }

        #[cfg(not(target_os = "windows"))]
        {
            let provider = super::accessibility::FallbackAccessibility::new()?;
            provider.extract_interactive_elements()
        }
    }
}

fn parse_key(key_str: &str) -> Result<Key> {
    match key_str {
        "enter" | "return" => Ok(Key::Return),
        "tab" => Ok(Key::Tab),
        "escape" | "esc" => Ok(Key::Escape),
        "backspace" => Ok(Key::Backspace),
        "delete" | "del" => Ok(Key::Delete),
        "space" => Ok(Key::Space),
        "up" | "arrowup" => Ok(Key::UpArrow),
        "down" | "arrowdown" => Ok(Key::DownArrow),
        "left" | "arrowleft" => Ok(Key::LeftArrow),
        "right" | "arrowright" => Ok(Key::RightArrow),
        "home" => Ok(Key::Home),
        "end" => Ok(Key::End),
        "pageup" => Ok(Key::PageUp),
        "pagedown" => Ok(Key::PageDown),
        "f1" => Ok(Key::F1),
        "f2" => Ok(Key::F2),
        "f3" => Ok(Key::F3),
        "f4" => Ok(Key::F4),
        "f5" => Ok(Key::F5),
        "f6" => Ok(Key::F6),
        "f7" => Ok(Key::F7),
        "f8" => Ok(Key::F8),
        "f9" => Ok(Key::F9),
        "f10" => Ok(Key::F10),
        "f11" => Ok(Key::F11),
        "f12" => Ok(Key::F12),
        s if s.len() == 1 => {
            let c = s.chars().next().unwrap();
            Ok(Key::Unicode(c))
        }
        _ => Err(anyhow!(
            "Unknown key: '{}'. Use: enter, tab, escape, backspace, delete, space, up, down, left, right, home, end, pageup, pagedown, f1-f12, or a single character.",
            key_str
        )),
    }
}

fn resize_to_fit(img: &RgbaImage, max_width: u32, max_height: u32) -> RgbaImage {
    if img.width() <= max_width && img.height() <= max_height {
        return img.clone();
    }

    let ratio_w = max_width as f64 / img.width() as f64;
    let ratio_h = max_height as f64 / img.height() as f64;
    let ratio = ratio_w.min(ratio_h);

    let new_width = (img.width() as f64 * ratio) as u32;
    let new_height = (img.height() as f64 * ratio) as u32;

    imageops::resize(img, new_width, new_height, imageops::FilterType::Lanczos3)
}

fn encode_jpeg_base64(img: &RgbaImage) -> Result<String> {
    let rgb_img = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();

    let mut buf = Cursor::new(Vec::new());
    let mut encoder = JpegEncoder::new_with_quality(&mut buf, JPEG_QUALITY);
    encoder
        .encode(
            &rgb_img,
            rgb_img.width(),
            rgb_img.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| anyhow!("Failed to encode JPEG: {}", e))?;

    Ok(base64::engine::general_purpose::STANDARD.encode(buf.into_inner()))
}

fn get_active_window_title() -> String {
    #[cfg(target_os = "windows")]
    {
        get_active_window_title_windows().unwrap_or_else(|| "Unknown".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        "Unknown".to_string()
    }
}

#[cfg(target_os = "windows")]
fn get_active_window_title_windows() -> Option<String> {
    // Use uiautomation's re-exported Windows types to avoid version conflicts
    let automation = uiautomation::UIAutomation::new().ok()?;
    let focused = automation.get_focused_element().ok()?;

    // Walk up to find the top-level window
    let walker = automation.create_tree_walker().ok()?;
    let root = automation.get_root_element().ok()?;
    let mut current = focused;

    loop {
        match walker.get_parent(&current) {
            Ok(parent) => {
                // Check if parent is root (desktop)
                if parent.get_name().unwrap_or_default()
                    == root.get_name().unwrap_or_default()
                {
                    break;
                }
                current = parent;
            }
            Err(_) => break,
        }
    }

    let name = current.get_name().unwrap_or_default();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}
