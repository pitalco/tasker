//! Grid overlay system for vision-based OS automation
//!
//! Creates a labeled grid overlay on screenshots so the vision LLM can identify
//! locations using grid cell references like "A5", "B12", etc.

use ab_glyph::{FontRef, PxScale};
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_line_segment_mut, draw_text_mut};

/// Grid overlay configuration and utilities
pub struct GridOverlay {
    /// Number of columns in the grid
    pub cols: u32,
    /// Number of rows in the grid
    pub rows: u32,
    /// Width of the screen/image
    pub screen_width: u32,
    /// Height of the screen/image
    pub screen_height: u32,
}

impl GridOverlay {
    /// Create a new grid overlay for the given screen dimensions
    ///
    /// Uses default 20x20 grid which provides good granularity for most screens
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        Self {
            cols: 20,
            rows: 20,
            screen_width,
            screen_height,
        }
    }

    /// Create a grid overlay with custom dimensions
    pub fn with_dimensions(screen_width: u32, screen_height: u32, cols: u32, rows: u32) -> Self {
        Self {
            cols,
            rows,
            screen_width,
            screen_height,
        }
    }

    /// Width of each cell in pixels
    pub fn cell_width(&self) -> u32 {
        self.screen_width / self.cols
    }

    /// Height of each cell in pixels
    pub fn cell_height(&self) -> u32 {
        self.screen_height / self.rows
    }

    /// Convert a grid cell reference to screen coordinates (center of cell)
    ///
    /// Format: "A5", "B12", "AA3" (row letter(s) + column number)
    /// - Row: A=0, B=1, ..., Z=25, AA=26, AB=27, ...
    /// - Column: 1-based, so "A1" is the top-left cell
    pub fn cell_to_coordinates(&self, cell: &str) -> anyhow::Result<(i32, i32)> {
        let cell = cell.trim().to_uppercase();

        // Parse the cell reference (e.g., "B5" -> row=1, col=4)
        let (row_part, col_part) = self.parse_cell_reference(&cell)?;

        // Validate bounds
        if row_part >= self.rows {
            anyhow::bail!(
                "Row '{}' is out of bounds (max row: {})",
                self.row_to_letter(row_part),
                self.row_to_letter(self.rows - 1)
            );
        }
        if col_part >= self.cols || col_part == 0 {
            anyhow::bail!(
                "Column '{}' is out of bounds (valid range: 1-{})",
                col_part + 1,
                self.cols
            );
        }

        // Calculate center of the cell
        let cell_w = self.cell_width();
        let cell_h = self.cell_height();
        let x = (col_part * cell_w) + (cell_w / 2);
        let y = (row_part * cell_h) + (cell_h / 2);

        Ok((x as i32, y as i32))
    }

    /// Parse cell reference like "B5" or "AA12" into (row_index, col_index)
    fn parse_cell_reference(&self, cell: &str) -> anyhow::Result<(u32, u32)> {
        let mut row_str = String::new();
        let mut col_str = String::new();

        for ch in cell.chars() {
            if ch.is_alphabetic() {
                if !col_str.is_empty() {
                    anyhow::bail!("Invalid cell reference format: '{}'", cell);
                }
                row_str.push(ch);
            } else if ch.is_numeric() {
                col_str.push(ch);
            }
        }

        if row_str.is_empty() || col_str.is_empty() {
            anyhow::bail!("Invalid cell reference format: '{}'", cell);
        }

        let row = self.letter_to_row(&row_str)?;
        let col: u32 = col_str
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid column number: '{}'", col_str))?;

        // Convert 1-based column to 0-based index
        Ok((row, col.saturating_sub(1)))
    }

    /// Convert letter(s) to row index (A=0, B=1, ..., Z=25, AA=26, ...)
    fn letter_to_row(&self, letters: &str) -> anyhow::Result<u32> {
        let mut result: u32 = 0;
        for ch in letters.chars() {
            if !ch.is_ascii_uppercase() {
                anyhow::bail!("Invalid row letter: '{}'", ch);
            }
            result = result * 26 + (ch as u32 - 'A' as u32);
            // For multi-letter, add 1 for each position except last
            if letters.len() > 1 {
                result += 1;
            }
        }
        // Adjust for multi-letter
        if letters.len() > 1 {
            result -= 1;
        }
        Ok(result)
    }

    /// Convert row index to letter(s) (0=A, 1=B, ..., 25=Z, 26=AA, ...)
    fn row_to_letter(&self, row: u32) -> String {
        let mut result = String::new();
        let mut n = row;

        loop {
            result.insert(0, (b'A' + (n % 26) as u8) as char);
            if n < 26 {
                break;
            }
            n = n / 26 - 1;
        }

        result
    }

    /// Generate a text description of the grid for the LLM prompt
    pub fn generate_grid_description(&self) -> String {
        let last_row = self.row_to_letter(self.rows - 1);
        format!(
            "The screen is divided into a {}x{} grid. \
             Rows are labeled A-{} (top to bottom). \
             Columns are numbered 1-{} (left to right). \
             Reference cells like 'A1' (top-left), '{}{1}' (bottom-left), \
             'A{}' (top-right), '{}{}' (bottom-right). \
             Each cell is approximately {}x{} pixels.",
            self.cols,
            self.rows,
            last_row,
            self.cols,
            last_row,
            self.cols,
            last_row,
            self.cols,
            self.cell_width(),
            self.cell_height()
        )
    }

    /// Draw grid lines on an image
    pub fn draw_grid_lines(&self, image: &mut RgbaImage, color: Rgba<u8>, thickness: u32) {
        let cell_w = self.cell_width();
        let cell_h = self.cell_height();

        // Draw vertical lines
        for i in 0..=self.cols {
            let x = (i * cell_w) as f32;
            for t in 0..thickness {
                let x_offset = x + t as f32;
                if x_offset < self.screen_width as f32 {
                    draw_line_segment_mut(
                        image,
                        (x_offset, 0.0),
                        (x_offset, self.screen_height as f32 - 1.0),
                        color,
                    );
                }
            }
        }

        // Draw horizontal lines
        for i in 0..=self.rows {
            let y = (i * cell_h) as f32;
            for t in 0..thickness {
                let y_offset = y + t as f32;
                if y_offset < self.screen_height as f32 {
                    draw_line_segment_mut(
                        image,
                        (0.0, y_offset),
                        (self.screen_width as f32 - 1.0, y_offset),
                        color,
                    );
                }
            }
        }
    }

    /// Draw grid labels on an image
    pub fn draw_grid_labels(&self, image: &mut RgbaImage, font_data: &[u8], color: Rgba<u8>) {
        let font = match FontRef::try_from_slice(font_data) {
            Ok(f) => f,
            Err(_) => return, // Silently skip if font can't be loaded
        };

        let cell_w = self.cell_width();
        let cell_h = self.cell_height();

        // Scale font based on cell size (aim for ~40% of cell height)
        let font_size = (cell_h as f32 * 0.35).max(8.0).min(20.0);
        let scale = PxScale::from(font_size);

        for row in 0..self.rows {
            for col in 0..self.cols {
                let label = format!("{}{}", self.row_to_letter(row), col + 1);

                // Position label in top-left corner of cell with small padding
                let x = (col * cell_w) as i32 + 2;
                let y = (row * cell_h) as i32 + 2;

                draw_text_mut(image, color, x, y, scale, &font, &label);
            }
        }
    }

    /// Draw a complete grid overlay on an image (lines + labels)
    ///
    /// If font_data is provided, labels will be drawn. Otherwise, only grid lines.
    pub fn draw_overlay(&self, image: &mut RgbaImage, font_data: Option<&[u8]>) {
        // Semi-transparent red for grid lines
        let line_color = Rgba([255, 0, 0, 128]);

        self.draw_grid_lines(image, line_color, 1);

        // Draw labels if font is provided
        if let Some(font) = font_data {
            let label_color = Rgba([255, 255, 255, 255]);
            self.draw_grid_labels(image, font, label_color);
        }
    }

    /// Draw a minimal grid overlay (just lines, no labels) - faster and less cluttered
    pub fn draw_overlay_minimal(&self, image: &mut RgbaImage) {
        let line_color = Rgba([255, 0, 0, 100]); // Semi-transparent red
        self.draw_grid_lines(image, line_color, 1);
    }
}

/// Options for grid overlay rendering
#[derive(Debug, Clone)]
pub struct GridOptions {
    /// Number of columns
    pub cols: u32,
    /// Number of rows
    pub rows: u32,
    /// Whether to draw labels
    pub show_labels: bool,
    /// Line color (RGBA)
    pub line_color: [u8; 4],
    /// Label color (RGBA)
    pub label_color: [u8; 4],
    /// Line thickness
    pub line_thickness: u32,
}

impl Default for GridOptions {
    fn default() -> Self {
        Self {
            cols: 20,
            rows: 20,
            show_labels: true,
            line_color: [255, 0, 0, 128], // Semi-transparent red
            label_color: [255, 255, 255, 255], // White
            line_thickness: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_to_coordinates() {
        let grid = GridOverlay::new(1920, 1080);

        // A1 should be top-left cell
        let (x, y) = grid.cell_to_coordinates("A1").unwrap();
        assert!(x > 0 && x < grid.cell_width() as i32);
        assert!(y > 0 && y < grid.cell_height() as i32);

        // Test case insensitivity
        let (x1, y1) = grid.cell_to_coordinates("a1").unwrap();
        let (x2, y2) = grid.cell_to_coordinates("A1").unwrap();
        assert_eq!((x1, y1), (x2, y2));
    }

    #[test]
    fn test_row_conversion() {
        let grid = GridOverlay::new(1920, 1080);

        assert_eq!(grid.row_to_letter(0), "A");
        assert_eq!(grid.row_to_letter(1), "B");
        assert_eq!(grid.row_to_letter(25), "Z");
        assert_eq!(grid.row_to_letter(26), "AA");
        assert_eq!(grid.row_to_letter(27), "AB");

        assert_eq!(grid.letter_to_row("A").unwrap(), 0);
        assert_eq!(grid.letter_to_row("B").unwrap(), 1);
        assert_eq!(grid.letter_to_row("Z").unwrap(), 25);
        assert_eq!(grid.letter_to_row("AA").unwrap(), 26);
    }

    #[test]
    fn test_grid_description() {
        let grid = GridOverlay::new(1920, 1080);
        let desc = grid.generate_grid_description();
        assert!(desc.contains("20x20"));
        assert!(desc.contains("A-T")); // 20 rows = A to T
    }

    #[test]
    fn test_invalid_cell() {
        let grid = GridOverlay::new(1920, 1080);

        // Out of bounds
        assert!(grid.cell_to_coordinates("Z99").is_err());
        // Invalid format
        assert!(grid.cell_to_coordinates("123").is_err());
        assert!(grid.cell_to_coordinates("ABC").is_err());
    }
}
