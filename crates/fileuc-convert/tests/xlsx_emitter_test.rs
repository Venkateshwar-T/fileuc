use fileuc_convert::ir::types::*;
use fileuc_convert::ir::xlsx_emitter::emit_xlsx;

#[test]
fn test_emit_basic_xlsx() {
    let mut doc = DocumentIR::new(Some("Test Spreadsheet".to_string()));
    let mut page = PageIR::new(1, 595.0, 842.0, PageOrientation::Portrait);

    // Create a 2x3 table with headers, numeric values, and text
    let cell_a1 = TableCellIR {
        row: 0,
        col: 0,
        row_span: 1,
        col_span: 1,
        bounds: Rect::new(50.0, 50.0, 100.0, 20.0),
        background_rgb: Some([224, 231, 255]),
        borders: CellBorders::default(),
        text_runs: vec![TextRunIR {
            text: "Product".to_string(),
            bounds: Rect::new(55.0, 55.0, 60.0, 12.0),
            font_family: "Calibri".to_string(),
            font_size_pt: 11.0,
            is_bold: true,
            is_italic: false,
            color_rgb: [30, 41, 59],
            char_spacing_pt: 0.0,
        }],
    };

    let cell_b1 = TableCellIR {
        row: 0,
        col: 1,
        row_span: 1,
        col_span: 1,
        bounds: Rect::new(150.0, 50.0, 80.0, 20.0),
        background_rgb: Some([224, 231, 255]),
        borders: CellBorders::default(),
        text_runs: vec![TextRunIR {
            text: "Quantity".to_string(),
            bounds: Rect::new(155.0, 55.0, 50.0, 12.0),
            font_family: "Calibri".to_string(),
            font_size_pt: 11.0,
            is_bold: true,
            is_italic: false,
            color_rgb: [30, 41, 59],
            char_spacing_pt: 0.0,
        }],
    };

    let cell_c1 = TableCellIR {
        row: 0,
        col: 2,
        row_span: 1,
        col_span: 1,
        bounds: Rect::new(230.0, 50.0, 80.0, 20.0),
        background_rgb: Some([224, 231, 255]),
        borders: CellBorders::default(),
        text_runs: vec![TextRunIR {
            text: "Price".to_string(),
            bounds: Rect::new(235.0, 55.0, 40.0, 12.0),
            font_family: "Calibri".to_string(),
            font_size_pt: 11.0,
            is_bold: true,
            is_italic: false,
            color_rgb: [30, 41, 59],
            char_spacing_pt: 0.0,
        }],
    };

    let cell_a2 = TableCellIR {
        row: 1,
        col: 0,
        row_span: 1,
        col_span: 1,
        bounds: Rect::new(50.0, 70.0, 100.0, 20.0),
        background_rgb: None,
        borders: CellBorders::default(),
        text_runs: vec![TextRunIR {
            text: "Widget Pro".to_string(),
            bounds: Rect::new(55.0, 75.0, 60.0, 12.0),
            font_family: "Calibri".to_string(),
            font_size_pt: 11.0,
            is_bold: false,
            is_italic: false,
            color_rgb: [15, 23, 42],
            char_spacing_pt: 0.0,
        }],
    };

    let cell_b2 = TableCellIR {
        row: 1,
        col: 1,
        row_span: 1,
        col_span: 1,
        bounds: Rect::new(150.0, 70.0, 80.0, 20.0),
        background_rgb: None,
        borders: CellBorders::default(),
        text_runs: vec![TextRunIR {
            text: "42".to_string(), // Inferred as numeric!
            bounds: Rect::new(155.0, 75.0, 20.0, 12.0),
            font_family: "Calibri".to_string(),
            font_size_pt: 11.0,
            is_bold: false,
            is_italic: false,
            color_rgb: [15, 23, 42],
            char_spacing_pt: 0.0,
        }],
    };

    let cell_c2 = TableCellIR {
        row: 1,
        col: 2,
        row_span: 1,
        col_span: 1,
        bounds: Rect::new(230.0, 70.0, 80.0, 20.0),
        background_rgb: None,
        borders: CellBorders::default(),
        text_runs: vec![TextRunIR {
            text: "$199.99".to_string(), // Inferred as numeric currency!
            bounds: Rect::new(235.0, 75.0, 45.0, 12.0),
            font_family: "Calibri".to_string(),
            font_size_pt: 11.0,
            is_bold: false,
            is_italic: false,
            color_rgb: [15, 23, 42],
            char_spacing_pt: 0.0,
        }],
    };

    page.tables.push(TableIR {
        bounds: Rect::new(50.0, 50.0, 260.0, 40.0),
        column_widths_pt: vec![100.0, 80.0, 80.0],
        cells: vec![cell_a1, cell_b1, cell_c1, cell_a2, cell_b2, cell_c2],
    });

    doc.pages.push(page);

    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let res = emit_xlsx(&doc, temp_file.path());
    assert!(res.is_ok(), "Failed to emit XLSX: {:?}", res.err());

    let file = std::fs::File::open(temp_file.path()).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    assert!(zip.by_name("xl/workbook.xml").is_ok());
    assert!(zip.by_name("xl/styles.xml").is_ok());
    assert!(zip.by_name("xl/worksheets/sheet1.xml").is_ok());
    assert!(zip.by_name("[Content_Types].xml").is_ok());
}
