use fileuc_convert::ir::pdfium_extractor::*;
use fileuc_convert::pdfium::init_pdfium;

#[test]
fn test_raw_content_types() {
    let raw = RawPageContent {
        page_number: 1,
        width_pt: 595.0,
        height_pt: 842.0,
        text_items: vec![RawTextItem {
            text: "Hello World".to_string(),
            x: 50.0,
            y: 100.0,
            width: 80.0,
            height: 12.0,
            font_family: "Arial".to_string(),
            font_size: 12.0,
            is_bold: false,
            is_italic: false,
            color_rgb: [0, 0, 0],
        }],
        paths: vec![RawPath {
            x1: 50.0,
            y1: 200.0,
            x2: 250.0,
            y2: 200.0,
            stroke_width: 1.0,
            stroke_color: Some([0, 0, 0]),
            fill_color: None,
        }],
        images: vec![],
    };

    assert_eq!(raw.text_items.len(), 1);
    assert_eq!(raw.text_items[0].text, "Hello World");
    assert_eq!(raw.paths.len(), 1);
    assert_eq!(raw.paths[0].x2, 250.0);
}

#[test]
fn test_extractor_signature() {
    let pdfium_res = init_pdfium();
    assert!(pdfium_res.is_ok());
}
