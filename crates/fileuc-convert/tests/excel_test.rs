use fileuc_convert::excel::{excel_to_pdf, render_table_to_pdf};

#[test]
fn test_csv_to_pdf_conversion() {
    let tmp = tempfile::tempdir().unwrap();
    let csv_path = tmp.path().join("sample.csv");
    let content = "ID,Name,Department,Salary,Start Date\n\
                   1,Alice Smith,Engineering,$120000,2021-03-15\n\
                   2,Bob Jones,Design,$110000,2022-01-10\n\
                   3,Carol White,Marketing,$95000,2020-07-22\n\
                   4,David Green,Sales,$105000,2019-11-05\n";
    std::fs::write(&csv_path, content).unwrap();

    let pdf_out = tmp.path().join("sample.pdf");
    excel_to_pdf(&csv_path, &pdf_out).expect("excel_to_pdf should succeed");
    assert!(pdf_out.exists());

    let bytes = std::fs::read(&pdf_out).unwrap();
    assert_eq!(&bytes[0..5], b"%PDF-");

    // Load with lopdf to ensure document structure is completely valid
    let doc = lopdf::Document::load(&pdf_out).expect("Should load valid PDF");
    assert_eq!(doc.get_pages().len(), 1, "Should be 1 page");
}

#[test]
fn test_wide_table_splits_into_horizontal_column_bands() {
    let tmp = tempfile::tempdir().unwrap();
    let pdf_out = tmp.path().join("wide_table.pdf");

    // Create a 20-column wide table that definitely exceeds 770pt A4 landscape usable width
    let mut header = Vec::new();
    for col in 1..=20 {
        header.push(format!("Column Header {:02}", col));
    }

    let mut rows = vec![header];
    for row_idx in 1..=10 {
        let mut row = Vec::new();
        for col_idx in 1..=20 {
            row.push(format!("Val R{}C{}", row_idx, col_idx));
        }
        rows.push(row);
    }

    render_table_to_pdf(&rows, &pdf_out).expect("render_table_to_pdf should succeed");
    assert!(pdf_out.exists());

    let doc = lopdf::Document::load(&pdf_out).expect("Should load valid PDF");
    let pages = doc.get_pages();
    println!("Wide table generated {} pages across column bands", pages.len());
    assert!(pages.len() >= 2, "Wide table should split across at least 2 horizontal column bands");
}
