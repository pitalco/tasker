use ab_glyph::{FontArc, PxScale};
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut};
use imageproc::rect::Rect as ImgRect;
use std::sync::OnceLock;

use super::accessibility::DesktopElement;

/// Font size for index labels
const LABEL_FONT_SIZE: f32 = 11.0;
/// Background color for labels (semi-transparent red/orange)
const LABEL_BG_COLOR: Rgba<u8> = Rgba([220, 50, 50, 200]);
/// Text color for labels (white)
const LABEL_TEXT_COLOR: Rgba<u8> = Rgba([255, 255, 255, 255]);
/// Padding around text in label
const LABEL_PADDING_X: i32 = 3;
const LABEL_PADDING_Y: i32 = 1;

/// Cache the loaded font globally so we only load once
static FONT_CACHE: OnceLock<FontArc> = OnceLock::new();

/// Load font from system paths (Windows -> Consolas, macOS -> Menlo, Linux -> DejaVu)
fn load_system_font() -> Option<FontArc> {
    let font_paths: &[&str] = &[
        // Windows
        "C:\\Windows\\Fonts\\consola.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
        // macOS
        "/System/Library/Fonts/Menlo.ttc",
        "/System/Library/Fonts/SFNSMono.ttf",
        "/Library/Fonts/Arial.ttf",
        // Linux
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
    ];

    for path in font_paths {
        if let Ok(data) = std::fs::read(path) {
            if let Ok(font) = FontArc::try_from_vec(data) {
                return Some(font);
            }
        }
    }
    None
}

/// Get the font for rendering labels (loaded once, cached)
fn get_font() -> &'static FontArc {
    FONT_CACHE.get_or_init(|| {
        load_system_font().expect(
            "Failed to load any system font. Ensure a TTF font is available on the system.",
        )
    })
}

/// Annotate a screenshot with numbered markers at each interactive element's position.
///
/// This implements the "Set-of-Marks" approach: small colored labels [1], [2], [3]...
/// are drawn at the top-left corner of each element's bounding box so the LLM
/// can visually match indices to UI elements.
pub fn annotate_screenshot(
    img: &RgbaImage,
    elements: &[DesktopElement],
    scale_factor: f64,
) -> RgbaImage {
    let mut annotated = img.clone();
    let font = get_font();
    let scale = PxScale::from(LABEL_FONT_SIZE);

    for elem in elements {
        // Scale element bounds to screenshot coordinates
        let x = (elem.bounds.x * scale_factor) as i32;
        let y = (elem.bounds.y * scale_factor) as i32;

        draw_index_label(&mut annotated, font, scale, elem.index, x, y);
    }

    annotated
}

/// Draw a small index label (e.g., "5") with a colored background at the given position
fn draw_index_label(
    img: &mut RgbaImage,
    font: &FontArc,
    scale: PxScale,
    index: usize,
    x: i32,
    y: i32,
) {
    let label_text = index.to_string();
    let img_width = img.width() as i32;
    let img_height = img.height() as i32;

    // Estimate text dimensions
    let char_width = (LABEL_FONT_SIZE * 0.6) as i32;
    let text_width = char_width * label_text.len() as i32;
    let text_height = LABEL_FONT_SIZE as i32;

    let box_width = text_width + LABEL_PADDING_X * 2;
    let box_height = text_height + LABEL_PADDING_Y * 2;

    // Clamp position to stay within image bounds
    let box_x = x.max(0).min((img_width - box_width).max(0));
    let box_y = y.max(0).min((img_height - box_height).max(0));

    // Draw background rectangle
    if box_x + box_width <= img_width && box_y + box_height <= img_height {
        draw_filled_rect_mut(
            img,
            ImgRect::at(box_x, box_y).of_size(box_width as u32, box_height as u32),
            LABEL_BG_COLOR,
        );
    }

    // Draw text
    let text_x = box_x + LABEL_PADDING_X;
    let text_y = box_y + LABEL_PADDING_Y;

    if text_x >= 0 && text_y >= 0 {
        draw_text_mut(
            img,
            LABEL_TEXT_COLOR,
            text_x,
            text_y,
            scale,
            font,
            &label_text,
        );
    }
}
