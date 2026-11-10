use fileuc_convert::ir::pptx_emitter::emit_pptx;
use fileuc_convert::ir::types::*;

#[test]
fn test_emit_basic_pptx() {
    let mut doc = DocumentIR::new(Some("Test Presentation".to_string()));
    let mut page = PageIR::new(1, 960.0, 540.0, PageOrientation::Landscape);

    let run = TextRunIR {
        text: "Title of Slide".to_string(),
        bounds: Rect::new(50.0, 40.0, 400.0, 36.0),
        font_family: "Calibri".to_string(),
        font_size_pt: 28.0,
        is_bold: true,
        is_italic: false,
        color_rgb: [15, 23, 42],
        char_spacing_pt: 0.0,
    };

    let line = TextLineIR {
        bounds: Rect::new(50.0, 40.0, 400.0, 36.0),
        baseline_y: 68.0,
        runs: vec![run],
    };

    let para = ParagraphIR {
        bounds: Rect::new(50.0, 40.0, 400.0, 36.0),
        alignment: TextAlignment::Left,
        line_spacing_pt: 36.0,
        space_after_pt: 12.0,
        lines: vec![line],
    };

    page.blocks.push(BlockIR::Paragraph(para));

    // Add a native table
    let cell1 = TableCellIR {
        row: 0,
        col: 0,
        row_span: 1,
        col_span: 1,
        bounds: Rect::new(50.0, 120.0, 150.0, 30.0),
        background_rgb: Some([241, 245, 249]),
        borders: CellBorders::default(),
        text_runs: vec![TextRunIR {
            text: "Item 1".to_string(),
            bounds: Rect::new(55.0, 125.0, 60.0, 14.0),
            font_family: "Calibri".to_string(),
            font_size_pt: 12.0,
            is_bold: true,
            is_italic: false,
            color_rgb: [0, 0, 0],
            char_spacing_pt: 0.0,
        }],
    };

    let cell2 = TableCellIR {
        row: 0,
        col: 1,
        row_span: 1,
        col_span: 1,
        bounds: Rect::new(200.0, 120.0, 150.0, 30.0),
        background_rgb: Some([241, 245, 249]),
        borders: CellBorders::default(),
        text_runs: vec![TextRunIR {
            text: "Value 1".to_string(),
            bounds: Rect::new(205.0, 125.0, 60.0, 14.0),
            font_family: "Calibri".to_string(),
            font_size_pt: 12.0,
            is_bold: false,
            is_italic: false,
            color_rgb: [0, 0, 0],
            char_spacing_pt: 0.0,
        }],
    };

    page.tables.push(TableIR {
        bounds: Rect::new(50.0, 120.0, 300.0, 30.0),
        column_widths_pt: vec![150.0, 150.0],
        cells: vec![cell1, cell2],
    });

    doc.pages.push(page);

    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let res = emit_pptx(&doc, temp_file.path());
    assert!(res.is_ok(), "Failed to emit PPTX: {:?}", res.err());

    // Verify it is a valid zip archive containing slides
    let file = std::fs::File::open(temp_file.path()).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    assert!(zip.by_name("ppt/slides/slide1.xml").is_ok());
    assert!(zip.by_name("ppt/presentation.xml").is_ok());
    assert!(zip.by_name("[Content_Types].xml").is_ok());
}
