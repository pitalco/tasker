use anyhow::Result;
use async_trait::async_trait;
use calamine::{open_workbook_auto_from_rs, Data, Range, Reader};
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Cursor;

use super::filesystem_tools::validate_path;
use super::registry::{Tool, ToolContext, ToolDefinition, ToolResult};

// ============================================================================
// Helper: Parse a hex color string like "#4472C4" to rust_xlsxwriter Color
// ============================================================================
fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::RGB(
        ((r as u32) << 16) | ((g as u32) << 8) | (b as u32),
    ))
}

// ============================================================================
// Helper: Build a Format from cell JSON properties
// ============================================================================
fn build_format(cell: &Value) -> Format {
    let mut fmt = Format::new();

    if cell["bold"].as_bool().unwrap_or(false) {
        fmt = fmt.set_bold();
    }
    if cell["italic"].as_bool().unwrap_or(false) {
        fmt = fmt.set_italic();
    }
    if let Some(size) = cell["font_size"].as_f64() {
        fmt = fmt.set_font_size(size);
    }
    if let Some(color_str) = cell["font_color"].as_str() {
        if let Some(color) = parse_hex_color(color_str) {
            fmt = fmt.set_font_color(color);
        }
    }
    if let Some(bg_str) = cell["bg_color"].as_str() {
        if let Some(color) = parse_hex_color(bg_str) {
            fmt = fmt.set_background_color(color);
        }
    }
    if let Some(nf) = cell["number_format"].as_str() {
        fmt = fmt.set_num_format(nf);
    }
    if let Some(align) = cell["align"].as_str() {
        let a = match align.to_lowercase().as_str() {
            "center" => FormatAlign::Center,
            "right" => FormatAlign::Right,
            "left" => FormatAlign::Left,
            "top" => FormatAlign::Top,
            "bottom" => FormatAlign::Bottom,
            "center_across" => FormatAlign::CenterAcross,
            _ => FormatAlign::Left,
        };
        fmt = fmt.set_align(a);
    }
    if cell["border"].as_bool().unwrap_or(false) {
        fmt = fmt.set_border(FormatBorder::Thin);
    }

    fmt
}

// ============================================================================
// Helper: Parse a range string like "A1:F20" into (start_row, start_col, end_row, end_col)
// ============================================================================
fn parse_range(range_str: &str) -> Option<(u32, u16, u32, u16)> {
    let parts: Vec<&str> = range_str.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let (r1, c1) = parse_cell_ref(parts[0])?;
    let (r2, c2) = parse_cell_ref(parts[1])?;
    Some((r1, c1, r2, c2))
}

/// Parse "A1" -> (row=0, col=0), "B3" -> (row=2, col=1), etc.
fn parse_cell_ref(cell: &str) -> Option<(u32, u16)> {
    let cell = cell.trim();
    let mut col_part = String::new();
    let mut row_part = String::new();

    for ch in cell.chars() {
        if ch.is_ascii_alphabetic() {
            col_part.push(ch.to_ascii_uppercase());
        } else if ch.is_ascii_digit() {
            row_part.push(ch);
        }
    }

    if col_part.is_empty() || row_part.is_empty() {
        return None;
    }

    // Convert column letters to 0-based index (A=0, B=1, ..., Z=25, AA=26, etc.)
    let mut col: u16 = 0;
    for ch in col_part.chars() {
        col = col * 26 + (ch as u16 - b'A' as u16 + 1);
    }
    col -= 1; // Make 0-based

    let row: u32 = row_part.parse::<u32>().ok()?.saturating_sub(1); // 1-based to 0-based

    Some((row, col))
}

/// Convert column index to letter(s): 0->A, 1->B, 25->Z, 26->AA
fn col_to_letter(col: u16) -> String {
    let mut result = String::new();
    let mut c = col as u32 + 1; // 1-based
    while c > 0 {
        c -= 1;
        result.insert(0, (b'A' + (c % 26) as u8) as char);
        c /= 26;
    }
    result
}

// ============================================================================
// CreateExcelTool
// ============================================================================

pub struct CreateExcelTool;

#[async_trait]
impl Tool for CreateExcelTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "create_excel".to_string(),
            description: "Create an Excel (.xlsx) workbook with multiple sheets, formulas, formatting, and structure. The file is written to disk (path must be within allowed directories). Supports: values (string/number/boolean), formulas (=SUM(...), =NPV(...), etc.), formatting (bold, italic, font_size, font_color, bg_color, number_format, align, border), column widths, row heights, merged cells, and freeze panes.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute file path for the workbook, e.g. '/home/user/output/model.xlsx'. Must be within allowed directories."
                    },
                    "sheets": {
                        "type": "array",
                        "description": "Array of sheet specifications",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {
                                    "type": "string",
                                    "description": "Sheet name"
                                },
                                "freeze_row": {
                                    "type": "integer",
                                    "description": "Freeze panes: rows above this row are frozen (0-based)"
                                },
                                "freeze_col": {
                                    "type": "integer",
                                    "description": "Freeze panes: columns left of this col are frozen (0-based)"
                                },
                                "columns": {
                                    "type": "array",
                                    "description": "Column width overrides",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "index": { "type": "integer" },
                                            "width": { "type": "number" }
                                        }
                                    }
                                },
                                "rows": {
                                    "type": "array",
                                    "description": "Row data",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "row": {
                                                "type": "integer",
                                                "description": "0-based row index (auto-increments if omitted)"
                                            },
                                            "height": {
                                                "type": "number",
                                                "description": "Row height in points"
                                            },
                                            "cells": {
                                                "type": "array",
                                                "description": "Cell data for this row",
                                                "items": {
                                                    "type": "object",
                                                    "properties": {
                                                        "col": { "type": "integer", "description": "0-based column index (auto-increments if omitted)" },
                                                        "value": { "description": "Cell value (string, number, or boolean)" },
                                                        "formula": { "type": "string", "description": "Excel formula (e.g. '=SUM(A1:A10)')" },
                                                        "bold": { "type": "boolean" },
                                                        "italic": { "type": "boolean" },
                                                        "font_size": { "type": "number" },
                                                        "font_color": { "type": "string", "description": "Hex color, e.g. '#FFFFFF'" },
                                                        "bg_color": { "type": "string", "description": "Hex background color" },
                                                        "number_format": { "type": "string", "description": "Excel number format, e.g. '#,##0', '0.0%'" },
                                                        "align": { "type": "string", "enum": ["left", "center", "right"] },
                                                        "border": { "type": "boolean", "description": "Add thin borders" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                },
                                "merge_ranges": {
                                    "type": "array",
                                    "description": "Cell ranges to merge, e.g. ['A1:C1']",
                                    "items": { "type": "string" }
                                }
                            },
                            "required": ["name"]
                        }
                    }
                },
                "required": ["file_path", "sheets"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let file_path = match params["file_path"].as_str() {
            Some(p) => p,
            None => return Ok(ToolResult::error("Missing 'file_path' parameter")),
        };

        let sheets = match params["sheets"].as_array() {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing 'sheets' array parameter")),
        };

        // Validate the path is within allowed directories
        let canonical = match validate_path(file_path, &ctx.allowed_directories) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(format!(
                "Invalid path '{}': {}. Path must be within allowed directories.", file_path, e
            ))),
        };

        let mut workbook = Workbook::new();
        let mut total_cells = 0u32;

        for sheet_spec in sheets {
            let sheet_name = sheet_spec["name"].as_str().unwrap_or("Sheet1");
            let worksheet = workbook.add_worksheet();
            worksheet.set_name(sheet_name)?;

            // Freeze panes
            let freeze_row = sheet_spec["freeze_row"].as_u64().unwrap_or(0) as u32;
            let freeze_col = sheet_spec["freeze_col"].as_u64().unwrap_or(0) as u16;
            if freeze_row > 0 || freeze_col > 0 {
                worksheet.set_freeze_panes(freeze_row, freeze_col)?;
            }

            // Column widths
            if let Some(columns) = sheet_spec["columns"].as_array() {
                for col_spec in columns {
                    if let (Some(idx), Some(width)) =
                        (col_spec["index"].as_u64(), col_spec["width"].as_f64())
                    {
                        worksheet.set_column_width(idx as u16, width)?;
                    }
                }
            }

            // Rows and cells
            if let Some(rows) = sheet_spec["rows"].as_array() {
                let mut auto_row: u32 = 0;
                for row_spec in rows {
                    let row_idx = row_spec["row"]
                        .as_u64()
                        .map(|r| r as u32)
                        .unwrap_or(auto_row);
                    auto_row = row_idx + 1;

                    // Row height
                    if let Some(height) = row_spec["height"].as_f64() {
                        worksheet.set_row_height(row_idx, height)?;
                    }

                    // Cells
                    if let Some(cells) = row_spec["cells"].as_array() {
                        let mut auto_col: u16 = 0;
                        for cell in cells {
                            let col_idx = cell["col"]
                                .as_u64()
                                .map(|c| c as u16)
                                .unwrap_or(auto_col);
                            auto_col = col_idx + 1;

                            let fmt = build_format(cell);
                            let has_format = cell["bold"].as_bool().unwrap_or(false)
                                || cell["italic"].as_bool().unwrap_or(false)
                                || cell["font_size"].is_number()
                                || cell["font_color"].is_string()
                                || cell["bg_color"].is_string()
                                || cell["number_format"].is_string()
                                || cell["align"].is_string()
                                || cell["border"].as_bool().unwrap_or(false);

                            if let Some(formula) = cell["formula"].as_str() {
                                if has_format {
                                    worksheet.write_formula_with_format(
                                        row_idx, col_idx, formula, &fmt,
                                    )?;
                                } else {
                                    worksheet.write_formula(row_idx, col_idx, formula)?;
                                }
                            } else if let Some(val) = cell.get("value") {
                                match val {
                                    Value::String(s) => {
                                        if has_format {
                                            worksheet.write_string_with_format(
                                                row_idx, col_idx, s, &fmt,
                                            )?;
                                        } else {
                                            worksheet.write_string(row_idx, col_idx, s)?;
                                        }
                                    }
                                    Value::Number(n) => {
                                        let num = n.as_f64().unwrap_or(0.0);
                                        if has_format {
                                            worksheet.write_number_with_format(
                                                row_idx, col_idx, num, &fmt,
                                            )?;
                                        } else {
                                            worksheet.write_number(row_idx, col_idx, num)?;
                                        }
                                    }
                                    Value::Bool(b) => {
                                        if has_format {
                                            worksheet.write_boolean_with_format(
                                                row_idx, col_idx, *b, &fmt,
                                            )?;
                                        } else {
                                            worksheet.write_boolean(row_idx, col_idx, *b)?;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            total_cells += 1;
                        }
                    }
                }
            }

            // Merge ranges
            if let Some(merges) = sheet_spec["merge_ranges"].as_array() {
                for merge in merges {
                    if let Some(range_str) = merge.as_str() {
                        if let Some((r1, c1, r2, c2)) = parse_range(range_str) {
                            worksheet.merge_range(
                                r1, c1, r2, c2, "",
                                &Format::new(),
                            )?;
                        }
                    }
                }
            }
        }

        // Save to buffer and write to disk
        let buffer = workbook.save_to_buffer()?;

        // Create parent directories if needed
        if let Some(parent) = canonical.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Ok(ToolResult::error(format!(
                    "Failed to create directories for '{}': {}", file_path, e
                )));
            }
        }

        match std::fs::write(&canonical, &buffer) {
            Ok(()) => Ok(ToolResult::success(format!(
                "Created Excel workbook '{}' with {} sheet(s) and {} cell(s)",
                file_path,
                sheets.len(),
                total_cells
            ))),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to save Excel file '{}': {}",
                file_path, e
            ))),
        }
    }
}

// ============================================================================
// ReadExcelTool
// ============================================================================

pub struct ReadExcelTool;

#[async_trait]
impl Tool for ReadExcelTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_excel".to_string(),
            description: "Read cell values from an Excel (.xlsx/.xls) file on disk. Path must be within allowed directories. Returns sheet names, dimensions, and cell data as a formatted text table.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute file path, e.g. '/home/user/output/model.xlsx'. Must be within allowed directories."
                    },
                    "sheet": {
                        "type": "string",
                        "description": "Sheet name to read (reads first sheet if omitted)"
                    },
                    "range": {
                        "type": "string",
                        "description": "Cell range to read, e.g. 'A1:F20' (reads all data if omitted)"
                    }
                },
                "required": ["file_path"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let file_path = match params["file_path"].as_str() {
            Some(p) => p,
            None => return Ok(ToolResult::error("Missing 'file_path' parameter")),
        };
        let sheet_name = params["sheet"].as_str();
        let range_str = params["range"].as_str();

        // Read from real filesystem
        let file_data: Vec<u8> = match validate_path(file_path, &ctx.allowed_directories) {
            Ok(canonical) if canonical.is_file() => {
                match std::fs::read(&canonical) {
                    Ok(bytes) => bytes,
                    Err(e) => return Ok(ToolResult::error(format!(
                        "Failed to read '{}': {}", file_path, e
                    ))),
                }
            }
            Ok(_) => return Ok(ToolResult::error(format!("Not a file: {}", file_path))),
            Err(e) => return Ok(ToolResult::error(format!(
                "Cannot access '{}': {}. Path must be within allowed directories.",
                file_path, e
            ))),
        };

        let cursor = Cursor::new(file_data);
        let mut workbook = match open_workbook_auto_from_rs(cursor) {
            Ok(wb) => wb,
            Err(e) => {
                return Ok(ToolResult::error(format!(
                    "Failed to open Excel file: {}",
                    e
                )))
            }
        };

        let sheet_names: Vec<String> = workbook.sheet_names().to_vec();

        // Determine which sheet to read
        let target_sheet = if let Some(name) = sheet_name {
            name.to_string()
        } else if let Some(first) = sheet_names.first() {
            first.clone()
        } else {
            return Ok(ToolResult::error("Workbook has no sheets"));
        };

        let range: Range<Data> = match workbook.worksheet_range(&target_sheet) {
            Ok(r) => r,
            Err(e) => {
                return Ok(ToolResult::error(format!(
                    "Sheet '{}' not found: {}",
                    target_sheet, e
                )))
            }
        };

        // Determine read bounds
        let (start_row, start_col, end_row, end_col) = if let Some(rs) = range_str {
            match parse_range(rs) {
                Some(r) => r,
                None => {
                    return Ok(ToolResult::error(format!(
                        "Invalid range format: '{}'. Use e.g. 'A1:F20'",
                        rs
                    )))
                }
            }
        } else {
            let (rows, cols) = range.get_size();
            if rows == 0 || cols == 0 {
                return Ok(ToolResult::success_with_data(
                    format!(
                        "Sheet '{}' is empty. Sheets: {:?}",
                        target_sheet, sheet_names
                    ),
                    json!({
                        "sheets": sheet_names,
                        "sheet": target_sheet,
                        "rows": 0,
                        "cols": 0,
                        "data": ""
                    }),
                ));
            }
            let (start_r, start_c) = range.start().unwrap_or((0, 0));
            (
                start_r as u32,
                start_c as u16,
                (start_r as u32 + rows as u32 - 1),
                (start_c as u16 + cols as u16 - 1),
            )
        };

        // Build text table
        let mut table = String::new();

        // Header row with column letters
        table.push_str("     |");
        for c in start_col..=end_col {
            table.push_str(&format!(" {:>12} |", col_to_letter(c)));
        }
        table.push('\n');

        // Separator
        table.push_str("-----+");
        for _ in start_col..=end_col {
            table.push_str("--------------+");
        }
        table.push('\n');

        let (range_start_row, range_start_col) = range.start().unwrap_or((0, 0));

        // Data rows
        for r in start_row..=end_row {
            table.push_str(&format!("{:>4} |", r + 1)); // 1-based display

            for c in start_col..=end_col {
                // calamine uses offsets relative to range start
                let rel_r = (r as usize).checked_sub(range_start_row as usize);
                let rel_c = (c as usize).checked_sub(range_start_col as usize);

                let cell_val = if let (Some(rr), Some(rc)) = (rel_r, rel_c) {
                    range.get((rr, rc))
                } else {
                    None
                };

                let display = match cell_val {
                    Some(Data::Empty) | None => String::new(),
                    Some(Data::String(s)) => s.to_string(),
                    Some(Data::Float(f)) => {
                        if *f == (*f as i64) as f64 && f.abs() < 1e15 {
                            format!("{}", *f as i64)
                        } else {
                            format!("{:.4}", f)
                        }
                    }
                    Some(Data::Int(i)) => format!("{}", i),
                    Some(Data::Bool(b)) => format!("{}", b),
                    Some(Data::Error(e)) => format!("ERR:{:?}", e),
                    Some(Data::DateTime(dt)) => format!("{}", dt),
                    Some(Data::DateTimeIso(s)) => s.to_string(),
                    Some(Data::DurationIso(s)) => s.to_string(),
                };

                // Truncate long values
                let truncated = if display.len() > 12 {
                    format!("{}...", &display[..9])
                } else {
                    display
                };

                table.push_str(&format!(" {:>12} |", truncated));
            }
            table.push('\n');
        }

        let (total_rows, total_cols) = range.get_size();
        let summary = format!(
            "Sheet '{}' [{} rows x {} cols]. Sheets: {:?}",
            target_sheet, total_rows, total_cols, sheet_names
        );

        Ok(ToolResult::success_with_data(
            format!("{}\n\n{}", summary, table),
            json!({
                "sheets": sheet_names,
                "sheet": target_sheet,
                "rows": total_rows,
                "cols": total_cols,
            }),
        ))
    }
}

// ============================================================================
// EditExcelTool
// ============================================================================

pub struct EditExcelTool;

/// Represents a cell value read from calamine that we'll write back
#[derive(Debug, Clone)]
enum CellValue {
    String(String),
    Float(f64),
    Int(i64),
    Bool(bool),
}

/// Read all cell data from the workbook using calamine
fn read_all_sheets(
    data: &[u8],
) -> Result<(Vec<String>, HashMap<String, Vec<(u32, u16, CellValue)>>)> {
    let cursor = Cursor::new(data);
    let mut workbook = open_workbook_auto_from_rs(cursor)
        .map_err(|e| anyhow::anyhow!("Failed to open workbook: {}", e))?;

    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
    let mut all_data: HashMap<String, Vec<(u32, u16, CellValue)>> = HashMap::new();

    for name in &sheet_names {
        let range = workbook
            .worksheet_range(name)
            .map_err(|e| anyhow::anyhow!("Failed to read sheet '{}': {}", name, e))?;

        let mut cells = Vec::new();
        let (start_row, start_col) = range.start().unwrap_or((0, 0));
        let (rows, cols) = range.get_size();

        for r in 0..rows {
            for c in 0..cols {
                if let Some(cell) = range.get((r, c)) {
                    let val = match cell {
                        Data::Empty => continue,
                        Data::String(s) => CellValue::String(s.clone()),
                        Data::Float(f) => CellValue::Float(*f),
                        Data::Int(i) => CellValue::Int(*i),
                        Data::Bool(b) => CellValue::Bool(*b),
                        Data::DateTime(dt) => {
                            // Convert ExcelDateTime to a float serial date if possible,
                            // otherwise store as string
                            CellValue::String(format!("{}", dt))
                        }
                        Data::DateTimeIso(s) => CellValue::String(s.clone()),
                        Data::DurationIso(s) => CellValue::String(s.clone()),
                        Data::Error(_) => CellValue::String("#ERROR".to_string()),
                    };
                    cells.push((
                        (start_row as u32) + r as u32,
                        (start_col as u16) + c as u16,
                        val,
                    ));
                }
            }
        }

        all_data.insert(name.clone(), cells);
    }

    Ok((sheet_names, all_data))
}

#[async_trait]
impl Tool for EditExcelTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "edit_excel".to_string(),
            description: "Edit specific cells in an existing Excel (.xlsx) file without resending the full specification. Reads the current workbook from disk, applies your cell changes (values, formulas, and formatting), and saves back. Path must be within allowed directories.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute file path of the existing Excel file. Must be within allowed directories."
                    },
                    "changes": {
                        "type": "array",
                        "description": "Array of cell changes to apply",
                        "items": {
                            "type": "object",
                            "properties": {
                                "sheet": {
                                    "type": "string",
                                    "description": "Sheet name (uses first sheet if omitted)"
                                },
                                "row": {
                                    "type": "integer",
                                    "description": "0-based row index"
                                },
                                "col": {
                                    "type": "integer",
                                    "description": "0-based column index"
                                },
                                "value": {
                                    "description": "New cell value (string, number, or boolean)"
                                },
                                "formula": {
                                    "type": "string",
                                    "description": "Excel formula (e.g. '=B5*1.1')"
                                },
                                "bold": { "type": "boolean" },
                                "italic": { "type": "boolean" },
                                "font_size": { "type": "number" },
                                "font_color": { "type": "string" },
                                "bg_color": { "type": "string" },
                                "number_format": { "type": "string" },
                                "align": { "type": "string" },
                                "border": { "type": "boolean" }
                            },
                            "required": ["row", "col"]
                        }
                    }
                },
                "required": ["file_path", "changes"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let file_path = match params["file_path"].as_str() {
            Some(p) => p,
            None => return Ok(ToolResult::error("Missing 'file_path' parameter")),
        };
        let changes = match params["changes"].as_array() {
            Some(c) => c,
            None => return Ok(ToolResult::error("Missing 'changes' array parameter")),
        };

        // Read existing file from real filesystem
        let canonical = match validate_path(file_path, &ctx.allowed_directories) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(format!(
                "Cannot access '{}': {}. Path must be within allowed directories.", file_path, e
            ))),
        };

        if !canonical.is_file() {
            return Ok(ToolResult::error(format!("Not a file: {}", file_path)));
        }

        let file_data = match std::fs::read(&canonical) {
            Ok(bytes) => bytes,
            Err(e) => return Ok(ToolResult::error(format!(
                "Failed to read '{}': {}", file_path, e
            ))),
        };

        // Parse existing workbook
        let (sheet_names, all_data) = match read_all_sheets(&file_data) {
            Ok(d) => d,
            Err(e) => {
                return Ok(ToolResult::error(format!(
                    "Failed to parse existing Excel file: {}",
                    e
                )))
            }
        };

        // Build a map of changes by sheet -> (row, col) -> change spec
        let mut change_map: HashMap<String, HashMap<(u32, u16), &Value>> = HashMap::new();
        let default_sheet = sheet_names.first().cloned().unwrap_or_default();

        for change in changes {
            let sheet = change["sheet"]
                .as_str()
                .unwrap_or(&default_sheet)
                .to_string();
            let row = change["row"].as_u64().unwrap_or(0) as u32;
            let col = change["col"].as_u64().unwrap_or(0) as u16;

            change_map
                .entry(sheet)
                .or_default()
                .insert((row, col), change);
        }

        // Rebuild workbook
        let mut workbook = Workbook::new();

        for sheet_name in &sheet_names {
            let worksheet = workbook.add_worksheet();
            worksheet.set_name(sheet_name)?;

            let sheet_changes = change_map.get(sheet_name);

            // Write existing cells (skip those being changed)
            if let Some(cells) = all_data.get(sheet_name) {
                for (row, col, val) in cells {
                    // Check if this cell is being overwritten by a change
                    if let Some(sc) = sheet_changes {
                        if sc.contains_key(&(*row, *col)) {
                            continue; // Skip - will be written by the change
                        }
                    }

                    match val {
                        CellValue::String(s) => {
                            worksheet.write_string(*row, *col, s)?;
                        }
                        CellValue::Float(f) => {
                            worksheet.write_number(*row, *col, *f)?;
                        }
                        CellValue::Int(i) => {
                            worksheet.write_number(*row, *col, *i as f64)?;
                        }
                        CellValue::Bool(b) => {
                            worksheet.write_boolean(*row, *col, *b)?;
                        }
                    }
                }
            }

            // Write changes for this sheet
            if let Some(sc) = sheet_changes {
                for ((row, col), change) in sc {
                    let fmt = build_format(change);
                    let has_format = change["bold"].as_bool().unwrap_or(false)
                        || change["italic"].as_bool().unwrap_or(false)
                        || change["font_size"].is_number()
                        || change["font_color"].is_string()
                        || change["bg_color"].is_string()
                        || change["number_format"].is_string()
                        || change["align"].is_string()
                        || change["border"].as_bool().unwrap_or(false);

                    if let Some(formula) = change["formula"].as_str() {
                        if has_format {
                            worksheet.write_formula_with_format(*row, *col, formula, &fmt)?;
                        } else {
                            worksheet.write_formula(*row, *col, formula)?;
                        }
                    } else if let Some(val) = change.get("value") {
                        match val {
                            Value::String(s) => {
                                if has_format {
                                    worksheet.write_string_with_format(*row, *col, s, &fmt)?;
                                } else {
                                    worksheet.write_string(*row, *col, s)?;
                                }
                            }
                            Value::Number(n) => {
                                let num = n.as_f64().unwrap_or(0.0);
                                if has_format {
                                    worksheet.write_number_with_format(*row, *col, num, &fmt)?;
                                } else {
                                    worksheet.write_number(*row, *col, num)?;
                                }
                            }
                            Value::Bool(b) => {
                                if has_format {
                                    worksheet.write_boolean_with_format(*row, *col, *b, &fmt)?;
                                } else {
                                    worksheet.write_boolean(*row, *col, *b)?;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Also handle changes for sheets that might not exist yet (create them)
        for (sheet_name, changes) in &change_map {
            if !sheet_names.contains(sheet_name) {
                let worksheet = workbook.add_worksheet();
                worksheet.set_name(sheet_name)?;

                for ((row, col), change) in changes {
                    let fmt = build_format(change);
                    let has_format = change["bold"].as_bool().unwrap_or(false)
                        || change["italic"].as_bool().unwrap_or(false)
                        || change["font_size"].is_number()
                        || change["font_color"].is_string()
                        || change["bg_color"].is_string()
                        || change["number_format"].is_string()
                        || change["align"].is_string()
                        || change["border"].as_bool().unwrap_or(false);

                    if let Some(formula) = change["formula"].as_str() {
                        if has_format {
                            worksheet.write_formula_with_format(*row, *col, formula, &fmt)?;
                        } else {
                            worksheet.write_formula(*row, *col, formula)?;
                        }
                    } else if let Some(val) = change.get("value") {
                        match val {
                            Value::String(s) => {
                                if has_format {
                                    worksheet.write_string_with_format(*row, *col, s, &fmt)?;
                                } else {
                                    worksheet.write_string(*row, *col, s)?;
                                }
                            }
                            Value::Number(n) => {
                                let num = n.as_f64().unwrap_or(0.0);
                                if has_format {
                                    worksheet.write_number_with_format(*row, *col, num, &fmt)?;
                                } else {
                                    worksheet.write_number(*row, *col, num)?;
                                }
                            }
                            Value::Bool(b) => {
                                if has_format {
                                    worksheet.write_boolean_with_format(*row, *col, *b, &fmt)?;
                                } else {
                                    worksheet.write_boolean(*row, *col, *b)?;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Save to buffer and write back to disk
        let buffer = workbook.save_to_buffer()?;

        match std::fs::write(&canonical, &buffer) {
            Ok(()) => Ok(ToolResult::success(format!(
                "Updated '{}': applied {} change(s) across {} sheet(s)",
                file_path,
                changes.len(),
                change_map.len()
            ))),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to write '{}': {}", file_path, e
            ))),
        }
    }
}
