use fileuc_convert::ir::types::*;

#[test]
fn test_document_ir_creation() {
    let mut doc = DocumentIR::new(Some("Sample Document".to_string()));
    assert_eq!(doc.title.as_deref(), Some("Sample Document"));
    assert!(doc.pages.is_empty());

    let page = PageIR::new(1, 595.0, 842.0, PageOrientation::Portrait);
    doc.pages.push(page);
    assert_eq!(doc.pages.len(), 1);
    assert_eq!(doc.pages[0].width_pt, 595.0);
    assert_eq!(doc.pages[0].height_pt, 842.0);
    assert_eq!(doc.pages[0].orientation, PageOrientation::Portrait);
}

#[test]
fn test_paragraph_and_run_hierarchy() {
    let run = TextRunIR {
        text: "High Fidelity".to_string(),
        bounds: Rect::new(50.0, 100.0, 120.0, 14.0),
        font_family: "Calibri".to_string(),
        font_size_pt: 12.0,
        is_bold: true,
        is_italic: false,
        color_rgb: [15, 23, 42],
        char_spacing_pt: 0.0,
    };

    let line = TextLineIR {
        bounds: Rect::new(50.0, 100.0, 120.0, 14.0),
        baseline_y: 112.0,
        runs: vec![run],
    };

    let para = ParagraphIR {
        bounds: Rect::new(50.0, 100.0, 120.0, 14.0),
        alignment: TextAlignment::Left,
        line_spacing_pt: 14.0,
        space_after_pt: 6.0,
        lines: vec![line],
    };

    let block = BlockIR::Paragraph(para);
    match block {
        BlockIR::Paragraph(p) => {
            assert_eq!(p.lines.len(), 1);
            assert_eq!(p.lines[0].runs[0].text, "High Fidelity");
            assert!(p.lines[0].runs[0].is_bold);
        }
    }
}

#[test]
fn test_table_ir_structure() {
    let cell = TableCellIR {
        row: 0,
        col: 0,
        row_span: 1,
        col_span: 1,
        bounds: Rect::new(50.0, 200.0, 100.0, 30.0),
        background_rgb: Some([240, 240, 240]),
        borders: CellBorders::default(),
        text_runs: vec![TextRunIR {
            text: "Cell A1".to_string(),
            bounds: Rect::new(55.0, 205.0, 40.0, 12.0),
            font_family: "Calibri".to_string(),
            font_size_pt: 10.0,
            is_bold: false,
            is_italic: false,
            color_rgb: [0, 0, 0],
            char_spacing_pt: 0.0,
        }],
    };

    let table = TableIR {
        bounds: Rect::new(50.0, 200.0, 200.0, 60.0),
        column_widths_pt: vec![100.0, 100.0],
        cells: vec![cell],
    };

    assert_eq!(table.cells.len(), 1);
    assert_eq!(table.column_widths_pt.len(), 2);
    assert_eq!(table.cells[0].text_runs[0].text, "Cell A1");
}
