use crate::ir::pdfium_extractor::{RawPageContent, RawTextItem};
use crate::ir::types::*;

pub fn analyze_page_layout(raw: RawPageContent) -> PageIR {
    let orientation = if raw.width_pt > raw.height_pt {
        PageOrientation::Landscape
    } else {
        PageOrientation::Portrait
    };

    let mut page = PageIR::new(raw.page_number, raw.width_pt, raw.height_pt, orientation);

    // 1. Process Images
    for img in raw.images {
        page.images.push(ImageIR {
            bounds: Rect::new(img.x, img.y, img.width, img.height),
            data: img.data,
            format: ImageFormat::Png,
        });
    }

    // 2. Process Shapes and Lines
    for path in &raw.paths {
        let w = (path.x2 - path.x1).abs();
        let h = (path.y2 - path.y1).abs();
        let min_x = path.x1.min(path.x2);
        let min_y = path.y1.min(path.y2);

        let shape_type = if w <= 3.0 || h <= 3.0 {
            ShapeType::Line
        } else {
            ShapeType::Rectangle
        };

        page.shapes.push(ShapeIR {
            bounds: Rect::new(min_x, min_y, w, h),
            shape_type,
            stroke_color: path.stroke_color,
            stroke_width_pt: path.stroke_width,
            fill_color: path.fill_color,
        });
    }

    // 3. Detect and extract Tables (Lattice)
    let mut text_items = raw.text_items;
    let tables = crate::ir::table_detector::detect_tables(&raw.paths, &mut text_items);
    page.tables = tables;

    // 4. Recursive XY-Cut & Reading Order Segmentation on remaining text
    if !text_items.is_empty() {
        let columns = partition_columns(text_items, raw.width_pt);
        for col_items in columns {
            let paragraphs = cluster_into_paragraphs(col_items);
            for p in paragraphs {
                page.blocks.push(BlockIR::Paragraph(p));
            }
        }
    }

    page
}

/// Partition text items into vertical column slices using vertical whitespace gutters
fn partition_columns(mut items: Vec<RawTextItem>, page_width: f32) -> Vec<Vec<RawTextItem>> {
    if items.len() <= 1 {
        return vec![items];
    }

    // Sort items by X coordinate
    items.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

    // Detect if there are vertical column gutters:
    // A gutter is a gap between the right edge of left items and left edge of right items
    // with width >= 20.0 pt spanning across multiple items.
    let mut column_boundaries = Vec::new();
    let num_bins = (page_width.max(100.0) as usize).min(1000);
    let mut occupancy = vec![false; num_bins];

    for item in &items {
        let start_bin = (item.x.clamp(0.0, page_width - 1.0) as usize).min(num_bins - 1);
        let end_bin = ((item.x + item.width).clamp(0.0, page_width - 1.0) as usize).min(num_bins - 1);
        for bin in &mut occupancy[start_bin..=end_bin] {
            *bin = true;
        }
    }

    // Find contiguous false runs (whitespace valleys) >= 20 pt wide not at page margins
    let margin_left = 30;
    let margin_right = num_bins.saturating_sub(30);

    let mut in_gap = false;
    let mut gap_start = 0;

    for i in margin_left..margin_right {
        if !occupancy[i] {
            if !in_gap {
                in_gap = true;
                gap_start = i;
            }
        } else if in_gap {
            in_gap = false;
            let gap_len = i - gap_start;
            if gap_len >= 20 {
                // Split point is center of gutter
                column_boundaries.push(gap_start as f32 + (gap_len as f32 / 2.0));
            }
        }
    }

    if column_boundaries.is_empty() {
        return vec![items];
    }

    // Assign items to columns
    let mut columns: Vec<Vec<RawTextItem>> = vec![Vec::new(); column_boundaries.len() + 1];

    for item in items {
        let center_x = item.x + (item.width / 2.0);
        let mut assigned_col = column_boundaries.len();
        for (col_idx, &bound) in column_boundaries.iter().enumerate() {
            if center_x < bound {
                assigned_col = col_idx;
                break;
            }
        }
        columns[assigned_col].push(item);
    }

    // Remove empty columns
    columns.into_iter().filter(|c| !c.is_empty()).collect()
}

/// Cluster items in a single column into lines, and lines into paragraphs
fn cluster_into_paragraphs(mut items: Vec<RawTextItem>) -> Vec<ParagraphIR> {
    if items.is_empty() {
        return Vec::new();
    }

    // 1. Sort primarily by Y, then by X
    items.sort_by(|a, b| {
        if (a.y - b.y).abs() <= 2.5 {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    // 2. Line clustering
    let mut lines: Vec<TextLineIR> = Vec::new();
    let mut cur_line_items: Vec<RawTextItem> = Vec::new();
    let mut cur_line_y = 0.0;
    let mut cur_line_height = 0.0;

    for item in items {
        if cur_line_items.is_empty() {
            cur_line_y = item.y;
            cur_line_height = item.height;
            cur_line_items.push(item);
        } else {
            // Check vertical proximity (within 3.0 pt or overlapping)
            let y_diff = (item.y - cur_line_y).abs();
            if y_diff <= 3.0 || (item.y >= cur_line_y && item.y < cur_line_y + cur_line_height * 0.8) {
                cur_line_items.push(item);
            } else {
                lines.push(build_text_line(std::mem::take(&mut cur_line_items)));
                cur_line_y = item.y;
                cur_line_height = item.height;
                cur_line_items.push(item);
            }
        }
    }
    if !cur_line_items.is_empty() {
        lines.push(build_text_line(cur_line_items));
    }

    // 3. Paragraph clustering: group lines with consistent spacing
    let mut paragraphs = Vec::new();
    let mut cur_para_lines: Vec<TextLineIR> = Vec::new();

    for line in lines {
        if cur_para_lines.is_empty() {
            cur_para_lines.push(line);
        } else {
            let prev_bottom = cur_para_lines.last().unwrap().bounds.bottom();
            let gap = line.bounds.y - prev_bottom;
            let prev_height = cur_para_lines.last().unwrap().bounds.height;

            // If gap exceeds 1.8x line height or is negative/large, break paragraph
            if gap > prev_height * 1.8 || gap < -2.0 {
                paragraphs.push(build_paragraph(std::mem::take(&mut cur_para_lines)));
                cur_para_lines.push(line);
            } else {
                cur_para_lines.push(line);
            }
        }
    }

    if !cur_para_lines.is_empty() {
        paragraphs.push(build_paragraph(cur_para_lines));
    }

    paragraphs
}

fn build_text_line(mut items: Vec<RawTextItem>) -> TextLineIR {
    // Sort left to right
    items.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

    let min_x = items.iter().map(|i| i.x).fold(f32::INFINITY, f32::min);
    let min_y = items.iter().map(|i| i.y).fold(f32::INFINITY, f32::min);
    let max_x = items.iter().map(|i| i.x + i.width).fold(f32::NEG_INFINITY, f32::max);
    let max_y = items.iter().map(|i| i.y + i.height).fold(f32::NEG_INFINITY, f32::max);

    let baseline_y = min_y + ((max_y - min_y) * 0.8);

    let runs = items
        .into_iter()
        .map(|item| TextRunIR {
            text: item.text,
            bounds: Rect::new(item.x, item.y, item.width, item.height),
            font_family: item.font_family,
            font_size_pt: item.font_size,
            is_bold: item.is_bold,
            is_italic: item.is_italic,
            color_rgb: item.color_rgb,
            char_spacing_pt: 0.0,
        })
        .collect();

    TextLineIR {
        bounds: Rect::new(min_x, min_y, (max_x - min_x).max(1.0), (max_y - min_y).max(1.0)),
        baseline_y,
        runs,
    }
}

fn build_paragraph(lines: Vec<TextLineIR>) -> ParagraphIR {
    let min_x = lines.iter().map(|l| l.bounds.x).fold(f32::INFINITY, f32::min);
    let min_y = lines.iter().map(|l| l.bounds.y).fold(f32::INFINITY, f32::min);
    let max_x = lines.iter().map(|l| l.bounds.right()).fold(f32::NEG_INFINITY, f32::max);
    let max_y = lines.iter().map(|l| l.bounds.bottom()).fold(f32::NEG_INFINITY, f32::max);

    let line_spacing_pt = if lines.len() > 1 {
        (max_y - min_y) / lines.len() as f32
    } else {
        lines.first().map(|l| l.bounds.height * 1.2).unwrap_or(14.0)
    };

    ParagraphIR {
        bounds: Rect::new(min_x, min_y, (max_x - min_x).max(1.0), (max_y - min_y).max(1.0)),
        alignment: TextAlignment::Left,
        line_spacing_pt,
        space_after_pt: 6.0,
        lines,
    }
}
