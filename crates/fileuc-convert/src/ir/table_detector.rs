use crate::ir::pdfium_extractor::{RawPath, RawTextItem};
use crate::ir::types::*;

pub fn detect_tables(paths: &[RawPath], text_items: &mut Vec<RawTextItem>) -> Vec<TableIR> {
    let mut h_lines: Vec<(f32, f32, f32)> = Vec::new(); // (y, x_min, x_max)
    let mut v_lines: Vec<(f32, f32, f32)> = Vec::new(); // (x, y_min, y_max)

    for p in paths {
        let dx = (p.x2 - p.x1).abs();
        let dy = (p.y2 - p.y1).abs();

        if dy <= 2.5 && dx >= 20.0 {
            let y = (p.y1 + p.y2) / 2.0;
            let x_min = p.x1.min(p.x2);
            let x_max = p.x1.max(p.x2);
            h_lines.push((y, x_min, x_max));
        } else if dx <= 2.5 && dy >= 15.0 {
            let x = (p.x1 + p.x2) / 2.0;
            let y_min = p.y1.min(p.y2);
            let y_max = p.y1.max(p.y2);
            v_lines.push((x, y_min, y_max));
        }
    }

    if h_lines.len() < 2 || v_lines.len() < 2 {
        return Vec::new();
    }

    // Cluster Y coordinates
    h_lines.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut unique_ys: Vec<f32> = Vec::new();
    for (y, _, _) in &h_lines {
        if let Some(last_y) = unique_ys.last() {
            if (y - last_y).abs() > 3.0 {
                unique_ys.push(*y);
            }
        } else {
            unique_ys.push(*y);
        }
    }

    // Cluster X coordinates
    v_lines.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut unique_xs: Vec<f32> = Vec::new();
    for (x, _, _) in &v_lines {
        if let Some(last_x) = unique_xs.last() {
            if (x - last_x).abs() > 3.0 {
                unique_xs.push(*x);
            }
        } else {
            unique_xs.push(*x);
        }
    }

    if unique_ys.len() < 2 || unique_xs.len() < 2 {
        return Vec::new();
    }

    let min_table_x = unique_xs.first().copied().unwrap_or(0.0);
    let max_table_x = unique_xs.last().copied().unwrap_or(0.0);
    let min_table_y = unique_ys.first().copied().unwrap_or(0.0);
    let max_table_y = unique_ys.last().copied().unwrap_or(0.0);

    let num_rows = unique_ys.len() - 1;
    let num_cols = unique_xs.len() - 1;

    let mut col_widths: Vec<f32> = Vec::with_capacity(num_cols);
    for c in 0..num_cols {
        col_widths.push((unique_xs[c + 1] - unique_xs[c]).max(10.0));
    }

    let mut cells: Vec<TableCellIR> = Vec::new();

    // Map cells
    for r in 0..num_rows {
        let cell_top = unique_ys[r];
        let cell_bottom = unique_ys[r + 1];
        let cell_h = cell_bottom - cell_top;

        for c in 0..num_cols {
            let cell_left = unique_xs[c];
            let cell_right = unique_xs[c + 1];
            let cell_w = cell_right - cell_left;

            let cell_bounds = Rect::new(cell_left, cell_top, cell_w, cell_h);

            cells.push(TableCellIR {
                row: r,
                col: c,
                row_span: 1,
                col_span: 1,
                bounds: cell_bounds,
                background_rgb: None,
                borders: CellBorders {
                    top: Some(BorderLine { width_pt: 1.0, color_rgb: [100, 116, 139] }),
                    right: Some(BorderLine { width_pt: 1.0, color_rgb: [100, 116, 139] }),
                    bottom: Some(BorderLine { width_pt: 1.0, color_rgb: [100, 116, 139] }),
                    left: Some(BorderLine { width_pt: 1.0, color_rgb: [100, 116, 139] }),
                },
                text_runs: Vec::new(),
            });
        }
    }

    // Assign text items to cells
    let mut remaining_text_items: Vec<RawTextItem> = Vec::new();

    for item in text_items.drain(..) {
        let center_x = item.x + (item.width / 2.0);
        let center_y = item.y + (item.height / 2.0);

        let mut matched_cell = false;

        for cell in &mut cells {
            if cell.bounds.contains_point(center_x, center_y) {
                cell.text_runs.push(TextRunIR {
                    text: item.text.clone(),
                    bounds: Rect::new(item.x, item.y, item.width, item.height),
                    font_family: item.font_family.clone(),
                    font_size_pt: item.font_size,
                    is_bold: item.is_bold,
                    is_italic: item.is_italic,
                    color_rgb: item.color_rgb,
                    char_spacing_pt: 0.0,
                });
                matched_cell = true;
                break;
            }
        }

        if !matched_cell {
            remaining_text_items.push(item);
        }
    }

    *text_items = remaining_text_items;

    let table = TableIR {
        bounds: Rect::new(
            min_table_x,
            min_table_y,
            max_table_x - min_table_x,
            max_table_y - min_table_y,
        ),
        column_widths_pt: col_widths,
        cells,
    };

    vec![table]
}
