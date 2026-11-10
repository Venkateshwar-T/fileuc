use fileuc_convert::ir::docx_emitter::emit_docx;
use fileuc_convert::ir::types::*;

#[test]
fn test_emit_basic_docx() {
    let mut doc = DocumentIR::new(Some("Test Word Doc".to_string()));
    let mut page = PageIR::new(1, 595.0, 842.0, PageOrientation::Portrait);

    let run1 = TextRunIR {
        text: "Document Heading".to_string(),
        bounds: Rect::new(72.0, 72.0, 300.0, 24.0),
        font_family: "Calibri".to_string(),
        font_size_pt: 18.0,
        is_bold: true,
        is_italic: false,
        color_rgb: [30, 41, 59],
        char_spacing_pt: 0.0,
    };

    let line1 = TextLineIR {
        bounds: Rect::new(72.0, 72.0, 300.0, 24.0),
        baseline_y: 90.0,
        runs: vec![run1],
    };

    let para1 = ParagraphIR {
        bounds: Rect::new(72.0, 72.0, 300.0, 24.0),
        alignment: TextAlignment::Left,
        line_spacing_pt: 24.0,
        space_after_pt: 12.0,
        lines: vec![line1],
    };

    page.blocks.push(BlockIR::Paragraph(para1));

    // Native Word Table
    let cell1 = TableCellIR {
        row: 0,
        col: 0,
        row_span: 1,
        col_span: 1,
        bounds: Rect::new(72.0, 120.0, 200.0, 28.0),
        background_rgb: Some([248, 250, 252]),
        borders: CellBorders::default(),
        text_runs: vec![TextRunIR {
            text: "Description".to_string(),
            bounds: Rect::new(76.0, 125.0, 80.0, 12.0),
            font_family: "Calibri".to_string(),
            font_size_pt: 11.0,
            is_bold: true,
            is_italic: false,
            color_rgb: [15, 23, 42],
            char_spacing_pt: 0.0,
        }],
    };

    let cell2 = TableCellIR {
        row: 0,
        col: 1,
        row_span: 1,
        col_span: 1,
        bounds: Rect::new(272.0, 120.0, 200.0, 28.0),
        background_rgb: Some([248, 250, 252]),
        borders: CellBorders::default(),
        text_runs: vec![TextRunIR {
            text: "Amount".to_string(),
            bounds: Rect::new(276.0, 125.0, 60.0, 12.0),
            font_family: "Calibri".to_string(),
            font_size_pt: 11.0,
            is_bold: true,
            is_italic: false,
            color_rgb: [15, 23, 42],
            char_spacing_pt: 0.0,
        }],
    };

    page.tables.push(TableIR {
        bounds: Rect::new(72.0, 120.0, 400.0, 28.0),
        column_widths_pt: vec![200.0, 200.0],
        cells: vec![cell1, cell2],
    });

    doc.pages.push(page);

    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let res = emit_docx(&doc, temp_file.path());
    assert!(res.is_ok(), "Failed to emit DOCX: {:?}", res.err());

    let file = std::fs::File::open(temp_file.path()).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    assert!(zip.by_name("word/document.xml").is_ok());
    assert!(zip.by_name("word/styles.xml").is_ok());
    assert!(zip.by_name("[Content_Types].xml").is_ok());
}
