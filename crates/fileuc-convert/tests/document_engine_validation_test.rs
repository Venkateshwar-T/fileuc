use fileuc_convert::ir::docx_emitter::emit_docx;
use fileuc_convert::ir::pptx_emitter::emit_pptx;
use fileuc_convert::ir::xlsx_emitter::emit_xlsx;
use fileuc_convert::ir::types::*;
use std::io::Read;

fn create_benchmark_document() -> DocumentIR {
    let mut doc = DocumentIR::new(Some("Financial & Performance Report".to_string()));
    let mut page = PageIR::new(1, 595.0, 842.0, PageOrientation::Portrait);

    // 1. Heading block
    let heading_run = TextRunIR {
        text: "Quarterly Performance & Financial Audit".to_string(),
        bounds: Rect::new(54.0, 54.0, 480.0, 24.0),
        font_family: "Arial".to_string(),
        font_size_pt: 20.0,
        is_bold: true,
        is_italic: false,
        color_rgb: [15, 23, 42],
        char_spacing_pt: 0.0,
    };
    let heading_line = TextLineIR {
        bounds: Rect::new(54.0, 54.0, 480.0, 24.0),
        baseline_y: 74.0,
        runs: vec![heading_run],
    };
    page.blocks.push(BlockIR::Paragraph(ParagraphIR {
        bounds: Rect::new(54.0, 54.0, 480.0, 24.0),
        alignment: TextAlignment::Left,
        line_spacing_pt: 24.0,
        space_after_pt: 12.0,
        lines: vec![heading_line],
    }));

    // 2. Prose paragraphs (simulating two-column body text)
    let p1_run = TextRunIR {
        text: "Operations in Q3 exceeded baseline projections by 14.8% due to optimized pipelines & streamlined delivery schedules.".to_string(),
        bounds: Rect::new(54.0, 90.0, 230.0, 36.0),
        font_family: "Calibri".to_string(),
        font_size_pt: 11.0,
        is_bold: false,
        is_italic: false,
        color_rgb: [51, 65, 85],
        char_spacing_pt: 0.0,
    };
    page.blocks.push(BlockIR::Paragraph(ParagraphIR {
        bounds: Rect::new(54.0, 90.0, 230.0, 36.0),
        alignment: TextAlignment::Justified,
        line_spacing_pt: 14.0,
        space_after_pt: 8.0,
        lines: vec![TextLineIR {
            bounds: Rect::new(54.0, 90.0, 230.0, 36.0),
            baseline_y: 102.0,
            runs: vec![p1_run],
        }],
    }));

    // 3. Structured Data Table (3x3 grid)
    let headers = ["Asset Category", "Allocation", "Return Rate"];
    let rows_data = [
        ["Fixed Equities", "$1,450,000", "8.4%"],
        ["Venture Index", "$820,000", "14.2%"],
    ];

    let mut cells = Vec::new();

    // Row 0: Headers
    for (col_idx, &h) in headers.iter().enumerate() {
        cells.push(TableCellIR {
            row: 0,
            col: col_idx,
            row_span: 1,
            col_span: 1,
            bounds: Rect::new(54.0 + (col_idx as f32 * 160.0), 140.0, 160.0, 24.0),
            background_rgb: Some([241, 245, 249]),
            borders: CellBorders::default(),
            text_runs: vec![TextRunIR {
                text: h.to_string(),
                bounds: Rect::new(58.0 + (col_idx as f32 * 160.0), 145.0, 100.0, 14.0),
                font_family: "Calibri".to_string(),
                font_size_pt: 11.0,
                is_bold: true,
                is_italic: false,
                color_rgb: [15, 23, 42],
                char_spacing_pt: 0.0,
            }],
        });
    }

    // Rows 1-2: Data
    for (r_idx, row) in rows_data.iter().enumerate() {
        for (col_idx, &val) in row.iter().enumerate() {
            cells.push(TableCellIR {
                row: r_idx + 1,
                col: col_idx,
                row_span: 1,
                col_span: 1,
                bounds: Rect::new(54.0 + (col_idx as f32 * 160.0), 164.0 + (r_idx as f32 * 24.0), 160.0, 24.0),
                background_rgb: None,
                borders: CellBorders::default(),
                text_runs: vec![TextRunIR {
                    text: val.to_string(),
                    bounds: Rect::new(58.0 + (col_idx as f32 * 160.0), 169.0 + (r_idx as f32 * 24.0), 100.0, 14.0),
                    font_family: "Calibri".to_string(),
                    font_size_pt: 11.0,
                    is_bold: false,
                    is_italic: false,
                    color_rgb: [51, 65, 85],
                    char_spacing_pt: 0.0,
                }],
            });
        }
    }

    page.tables.push(TableIR {
        bounds: Rect::new(54.0, 140.0, 480.0, 72.0),
        column_widths_pt: vec![160.0, 160.0, 160.0],
        cells,
    });

    doc.pages.push(page);
    doc
}

#[test]
fn test_docx_reconstruction_fidelity_and_editability() {
    let doc = create_benchmark_document();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let res = emit_docx(&doc, tmp.path());
    assert!(res.is_ok(), "DOCX emission failed: {:?}", res.err());

    let file = std::fs::File::open(tmp.path()).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();

    let mut doc_xml = String::new();
    zip.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut doc_xml)
        .unwrap();

    // Non-negotiable requirement: Editable text, not full-page rasterization
    assert!(doc_xml.contains("Quarterly Performance &amp; Financial Audit"), "Document heading missing or unescaped");
    assert!(doc_xml.contains("Fixed Equities"), "Table text run missing");
    assert!(doc_xml.contains("w:tbl"), "Native Word table tag <w:tbl> missing");
    assert!(doc_xml.contains("w:gridCol"), "Table column grid missing");
    assert!(doc_xml.contains("w:tcPr"), "Table cell properties missing");
    assert!(doc_xml.contains("w:shd"), "Cell background shading missing");
}

#[test]
fn test_pptx_reconstruction_fidelity_and_editability() {
    let doc = create_benchmark_document();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let res = emit_pptx(&doc, tmp.path());
    assert!(res.is_ok(), "PPTX emission failed: {:?}", res.err());

    let file = std::fs::File::open(tmp.path()).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();

    let mut slide_xml = String::new();
    zip.by_name("ppt/slides/slide1.xml")
        .unwrap()
        .read_to_string(&mut slide_xml)
        .unwrap();

    // Non-negotiable requirement: Native PowerPoint shapes and editable text boxes
    assert!(slide_xml.contains("Quarterly Performance &amp; Financial Audit"), "Slide heading missing or unescaped");
    assert!(slide_xml.contains("p:txBody"), "Editable text box body missing");
    assert!(slide_xml.contains("a:tbl"), "Native DrawingML table missing");
    assert!(slide_xml.contains("a:gridCol"), "Table column dimensions missing");
    assert!(slide_xml.contains("Venture Index"), "Slide table data missing");
}

#[test]
fn test_xlsx_reconstruction_fidelity_and_editability() {
    let doc = create_benchmark_document();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let res = emit_xlsx(&doc, tmp.path());
    assert!(res.is_ok(), "XLSX emission failed: {:?}", res.err());

    let file = std::fs::File::open(tmp.path()).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();

    let mut sheet_xml = String::new();
    zip.by_name("xl/worksheets/sheet1.xml")
        .unwrap()
        .read_to_string(&mut sheet_xml)
        .unwrap();

    // Non-negotiable requirement: Structured cells, typed numeric/currency values
    assert!(sheet_xml.contains("<col min=\"1\""), "Optimized column widths missing");
    assert!(sheet_xml.contains("Asset Category"), "Header text missing");
    assert!(sheet_xml.contains("Fixed Equities"), "Data cell missing");
    // Verify numeric currency parsed to numeric cell type
    assert!(sheet_xml.contains("<v>1450000</v>"), "Currency value was not parsed into typed numeric value");
    assert!(sheet_xml.contains("<v>820000</v>"), "Numeric value missing");
}
