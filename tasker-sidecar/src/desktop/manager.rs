//! Desktop manager for coordinating screenshot, grid, and input operations

use super::{GridOverlay, InputController, MouseButton, ScreenCapture};
use image::RgbaImage;

/// Manages desktop automation with vision-based coordinate system
pub struct DesktopManager {
    input: InputController,
    grid: Option<GridOverlay>,
    /// Optional embedded font data for grid labels
    font_data: Option<Vec<u8>>,
}

impl DesktopManager {
    /// Create a new desktop manager
    pub fn new() -> anyhow::Result<Self> {
        let input = InputController::new()?;
        Ok(Self {
            input,
            grid: None,
            font_data: None,
        })
    }

    /// Set custom font data for grid labels
    pub fn set_font(&mut self, font_data: Vec<u8>) {
        self.font_data = Some(font_data);
    }

    /// Capture the primary screen and return raw image
    pub fn capture_screen(&self) -> anyhow::Result<RgbaImage> {
        ScreenCapture::capture_primary_screen()
    }

    /// Capture the primary screen with grid overlay
    ///
    /// Returns the image and a description of the grid for the LLM prompt
    pub fn capture_with_grid(&mut self) -> anyhow::Result<(RgbaImage, String)> {
        let mut image = ScreenCapture::capture_primary_screen()?;

        // Create or update grid based on screen dimensions
        let (width, height) = (image.width(), image.height());
        let grid = GridOverlay::new(width, height);

        // Draw grid overlay
        grid.draw_overlay(&mut image, self.font_data.as_deref());

        let description = grid.generate_grid_description();
        self.grid = Some(grid);

        Ok((image, description))
    }

    /// Capture screen with grid and return as base64 PNG
    pub fn capture_with_grid_base64(&mut self) -> anyhow::Result<(String, String)> {
        let (image, description) = self.capture_with_grid()?;
        let base64 = ScreenCapture::image_to_base64(&image)?;
        Ok((base64, description))
    }

    /// Capture screen without grid overlay, return as base64 PNG
    pub fn capture_screen_base64(&self) -> anyhow::Result<String> {
        let image = self.capture_screen()?;
        ScreenCapture::image_to_base64(&image)
    }

    /// Get screen dimensions
    pub fn screen_size(&self) -> anyhow::Result<(u32, u32)> {
        ScreenCapture::primary_screen_size()
    }

    // ============ Coordinate-based Input ============

    /// Click at a grid cell reference (e.g., "B5")
    pub fn click_cell(&mut self, cell: &str) -> anyhow::Result<()> {
        let grid = self.get_or_create_grid()?;
        let (x, y) = grid.cell_to_coordinates(cell)?;
        self.input.click_at(x, y, MouseButton::Left)
    }

    /// Double-click at a grid cell reference
    pub fn double_click_cell(&mut self, cell: &str) -> anyhow::Result<()> {
        let grid = self.get_or_create_grid()?;
        let (x, y) = grid.cell_to_coordinates(cell)?;
        self.input.double_click_at(x, y, MouseButton::Left)
    }

    /// Right-click at a grid cell reference
    pub fn right_click_cell(&mut self, cell: &str) -> anyhow::Result<()> {
        let grid = self.get_or_create_grid()?;
        let (x, y) = grid.cell_to_coordinates(cell)?;
        self.input.click_at(x, y, MouseButton::Right)
    }

    /// Move mouse to a grid cell reference
    pub fn move_to_cell(&mut self, cell: &str) -> anyhow::Result<()> {
        let grid = self.get_or_create_grid()?;
        let (x, y) = grid.cell_to_coordinates(cell)?;
        self.input.move_mouse(x, y)
    }

    /// Scroll at a grid cell reference
    pub fn scroll_at_cell(&mut self, cell: &str, dx: i32, dy: i32) -> anyhow::Result<()> {
        let grid = self.get_or_create_grid()?;
        let (x, y) = grid.cell_to_coordinates(cell)?;
        self.input.move_mouse(x, y)?;
        self.input.scroll(dx, dy)
    }

    /// Drag from one grid cell to another
    pub fn drag_cells(&mut self, from_cell: &str, to_cell: &str) -> anyhow::Result<()> {
        let grid = self.get_or_create_grid()?;
        let (from_x, from_y) = grid.cell_to_coordinates(from_cell)?;
        let (to_x, to_y) = grid.cell_to_coordinates(to_cell)?;
        self.input.drag(from_x, from_y, to_x, to_y)
    }

    // ============ Direct Coordinate Input ============

    /// Click at specific screen coordinates
    pub fn click_at(&mut self, x: i32, y: i32) -> anyhow::Result<()> {
        self.input.click_at(x, y, MouseButton::Left)
    }

    /// Double-click at specific screen coordinates
    pub fn double_click_at(&mut self, x: i32, y: i32) -> anyhow::Result<()> {
        self.input.double_click_at(x, y, MouseButton::Left)
    }

    /// Right-click at specific screen coordinates
    pub fn right_click_at(&mut self, x: i32, y: i32) -> anyhow::Result<()> {
        self.input.click_at(x, y, MouseButton::Right)
    }

    /// Move mouse to specific screen coordinates
    pub fn move_mouse(&mut self, x: i32, y: i32) -> anyhow::Result<()> {
        self.input.move_mouse(x, y)
    }

    /// Scroll at specific coordinates
    pub fn scroll_at(&mut self, x: i32, y: i32, dx: i32, dy: i32) -> anyhow::Result<()> {
        self.input.move_mouse(x, y)?;
        self.input.scroll(dx, dy)
    }

    /// Drag from one point to another
    pub fn drag(&mut self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> anyhow::Result<()> {
        self.input.drag(from_x, from_y, to_x, to_y)
    }

    // ============ Keyboard Input ============

    /// Type text at current cursor position
    pub fn type_text(&mut self, text: &str) -> anyhow::Result<()> {
        self.input.type_text(text)
    }

    /// Press a key combination (e.g., Ctrl+C)
    pub fn hotkey(&mut self, modifiers: &[super::Modifier], key: super::KeyCode) -> anyhow::Result<()> {
        self.input.hotkey(modifiers, key)
    }

    /// Press a single key
    pub fn key_press(&mut self, key: super::KeyCode) -> anyhow::Result<()> {
        self.input.key_press(key)
    }

    // ============ Helpers ============

    /// Get existing grid or create a new one based on screen size
    fn get_or_create_grid(&mut self) -> anyhow::Result<&GridOverlay> {
        if self.grid.is_none() {
            let (width, height) = ScreenCapture::primary_screen_size()?;
            self.grid = Some(GridOverlay::new(width, height));
        }
        Ok(self.grid.as_ref().unwrap())
    }

    /// Get access to the input controller for advanced operations
    pub fn input_mut(&mut self) -> &mut InputController {
        &mut self.input
    }

    /// List all available monitors
    pub fn list_monitors(&self) -> anyhow::Result<Vec<super::MonitorInfo>> {
        ScreenCapture::list_monitors()
    }

    /// List all visible windows
    pub fn list_windows(&self) -> anyhow::Result<Vec<super::WindowInfo>> {
        ScreenCapture::list_windows()
    }
}
