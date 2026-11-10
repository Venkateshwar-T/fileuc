use fileuc_convert::ir::layout_analyzer::*;
use fileuc_convert::ir::pdfium_extractor::{RawPageContent, RawTextItem};
use fileuc_convert::ir::types::*;

#[test]
fn test_line_clustering_horizontal() {
    let raw = RawPageContent {
        page_number: 1,
        width_pt: 595.0,
        height_pt: 842.0,
        text_items: vec![
            RawTextItem {
                text: "Hello".to_string(),
                x: 50.0,
                y: 100.0,
                width: 30.0,
                height: 12.0,
                font_family: "Calibri".to_string(),
                font_size: 12.0,
                is_bold: false,
                is_italic: false,
                color_rgb: [0, 0, 0],
            },
            RawTextItem {
                text: "World".to_string(),
                x: 85.0,
                y: 100.2, // Within 2.0 pt tolerance
                width: 35.0,
                height: 12.0,
                font_family: "Calibri".to_string(),
                font_size: 12.0,
                is_bold: false,
                is_italic: false,
                color_rgb: [0, 0, 0],
            },
        ],
        paths: vec![],
        images: vec![],
    };

    let page = analyze_page_layout(raw);
    assert_eq!(page.blocks.len(), 1);
    match &page.blocks[0] {
        BlockIR::Paragraph(p) => {
            assert_eq!(p.lines.len(), 1);
            assert_eq!(p.lines[0].runs.len(), 2);
            assert_eq!(p.lines[0].runs[0].text, "Hello");
            assert_eq!(p.lines[0].runs[1].text, "World");
        }
    }
}

#[test]
fn test_multi_column_xy_cut() {
    // Column 1 at x=50, Column 2 at x=320 (gap of >50pt)
    let raw = RawPageContent {
        page_number: 1,
        width_pt: 595.0,
        height_pt: 842.0,
        text_items: vec![
            RawTextItem {
                text: "Col1 Line1".to_string(),
                x: 50.0,
                y: 100.0,
                width: 100.0,
                height: 12.0,
                font_family: "Calibri".to_string(),
                font_size: 12.0,
                is_bold: false,
                is_italic: false,
                color_rgb: [0, 0, 0],
            },
            RawTextItem {
                text: "Col2 Line1".to_string(),
                x: 320.0,
                y: 100.0,
                width: 100.0,
                height: 12.0,
                font_family: "Calibri".to_string(),
                font_size: 12.0,
                is_bold: false,
                is_italic: false,
                color_rgb: [0, 0, 0],
            },
            RawTextItem {
                text: "Col1 Line2".to_string(),
                x: 50.0,
                y: 118.0,
                width: 100.0,
                height: 12.0,
                font_family: "Calibri".to_string(),
                font_size: 12.0,
                is_bold: false,
                is_italic: false,
                color_rgb: [0, 0, 0],
            },
        ],
        paths: vec![],
        images: vec![],
    };

    let page = analyze_page_layout(raw);
    // Col1 Line 1 and Line 2 should be read before Col2 Line 1
    assert!(page.blocks.len() >= 2);
    let (BlockIR::Paragraph(p1), BlockIR::Paragraph(p2)) = (&page.blocks[0], &page.blocks[1]);
    assert!(p1.lines[0].runs[0].text.contains("Col1"));
    assert!(p2.lines[0].runs[0].text.contains("Col2"));
}
