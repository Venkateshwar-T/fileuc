use fileuc_convert::ir::pdfium_extractor::{RawPath, RawTextItem};
use fileuc_convert::ir::table_detector::*;

#[test]
fn test_detect_grid_table() {
    // Construct a 2x2 table grid formed by 3 horizontal lines and 3 vertical lines:
    // Rows at y=100, y=130, y=160
    // Cols at x=50, x=150, x=250
    let paths = vec![
        // Horizontal lines
        RawPath { x1: 50.0, y1: 100.0, x2: 250.0, y2: 100.0, stroke_width: 1.0, stroke_color: Some([0, 0, 0]), fill_color: None },
        RawPath { x1: 50.0, y1: 130.0, x2: 250.0, y2: 130.0, stroke_width: 1.0, stroke_color: Some([0, 0, 0]), fill_color: None },
        RawPath { x1: 50.0, y1: 160.0, x2: 250.0, y2: 160.0, stroke_width: 1.0, stroke_color: Some([0, 0, 0]), fill_color: None },
        // Vertical lines
        RawPath { x1: 50.0, y1: 100.0, x2: 50.0, y2: 160.0, stroke_width: 1.0, stroke_color: Some([0, 0, 0]), fill_color: None },
        RawPath { x1: 150.0, y1: 100.0, x2: 150.0, y2: 160.0, stroke_width: 1.0, stroke_color: Some([0, 0, 0]), fill_color: None },
        RawPath { x1: 250.0, y1: 100.0, x2: 250.0, y2: 160.0, stroke_width: 1.0, stroke_color: Some([0, 0, 0]), fill_color: None },
    ];

    let mut text_items = vec![
        RawTextItem {
            text: "Header 1".to_string(),
            x: 60.0,
            y: 110.0,
            width: 50.0,
            height: 12.0,
            font_family: "Calibri".to_string(),
            font_size: 11.0,
            is_bold: true,
            is_italic: false,
            color_rgb: [0, 0, 0],
        },
        RawTextItem {
            text: "Header 2".to_string(),
            x: 160.0,
            y: 110.0,
            width: 50.0,
            height: 12.0,
            font_family: "Calibri".to_string(),
            font_size: 11.0,
            is_bold: true,
            is_italic: false,
            color_rgb: [0, 0, 0],
        },
        RawTextItem {
            text: "Data 1".to_string(),
            x: 60.0,
            y: 140.0,
            width: 40.0,
            height: 12.0,
            font_family: "Calibri".to_string(),
            font_size: 11.0,
            is_bold: false,
            is_italic: false,
            color_rgb: [0, 0, 0],
        },
        RawTextItem {
            text: "Outside Text".to_string(),
            x: 50.0,
            y: 200.0,
            width: 70.0,
            height: 12.0,
            font_family: "Calibri".to_string(),
            font_size: 11.0,
            is_bold: false,
            is_italic: false,
            color_rgb: [0, 0, 0],
        },
    ];

    let tables = detect_tables(&paths, &mut text_items);
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 4); // 2x2 = 4 cells
    assert_eq!(tables[0].column_widths_pt.len(), 2);
    assert_eq!(tables[0].cells[0].text_runs[0].text, "Header 1");
    assert_eq!(tables[0].cells[1].text_runs[0].text, "Header 2");

    // "Outside Text" should remain in text_items, while table cells are drained
    assert_eq!(text_items.len(), 1);
    assert_eq!(text_items[0].text, "Outside Text");
}
