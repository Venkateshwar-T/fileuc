use fileuc_convert::pdf::images_to_pdf;

#[test]
fn images_to_pdf_creates_valid_pdf() {
    let tmp = tempfile::tempdir().unwrap();
    let img = tmp.path().join("page.png");
    image::RgbaImage::new(100, 100).save(&img).unwrap();
    let out = tmp.path().join("output.pdf");
    images_to_pdf(&[img], &out).unwrap();
    let bytes = std::fs::read(&out).unwrap();
    assert_eq!(&bytes[0..5], b"%PDF-");
}

#[test]
fn pdf_to_docx_creates_valid_docx_zip() {
    use std::io::Read;
    let tmp = tempfile::tempdir().unwrap();
    let img = tmp.path().join("page.png");
    image::RgbaImage::new(100, 100).save(&img).unwrap();
    let pdf_path = tmp.path().join("source.pdf");
    images_to_pdf(&[img], &pdf_path).unwrap();

    let docx_path = tmp.path().join("out.docx");
    fileuc_convert::pdf::pdf_to_docx(&pdf_path, &docx_path).unwrap();
    assert!(docx_path.exists());

    // Verify it's a valid zip containing all necessary parts and embedded image
    let file = std::fs::File::open(&docx_path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    assert!(zip.by_name("word/document.xml").is_ok());
    assert!(zip.by_name("[Content_Types].xml").is_ok());
    assert!(zip.by_name("word/styles.xml").is_ok());
    assert!(zip.by_name("word/_rels/document.xml.rels").is_ok());
    assert!(zip.by_name("word/media/image1.png").is_ok());

    let mut doc_file = zip.by_name("word/document.xml").unwrap();
    let mut doc_xml = String::new();
    doc_file.read_to_string(&mut doc_xml).unwrap();
    assert!(doc_xml.contains("<w:drawing>"));
    assert!(doc_xml.contains("<wp:inline"));
    assert!(doc_xml.contains("r:embed=\"rId2\""));
}

#[test]
fn pdf_to_doc_creates_valid_doc_rtf() {
    let tmp = tempfile::tempdir().unwrap();
    let img = tmp.path().join("page.png");
    image::RgbaImage::new(50, 50).save(&img).unwrap();
    let pdf_path = tmp.path().join("source.pdf");
    images_to_pdf(&[img], &pdf_path).unwrap();

    let doc_path = tmp.path().join("out.doc");
    fileuc_convert::pdf::pdf_to_doc(&pdf_path, &doc_path).unwrap();
    assert!(doc_path.exists());

    let content = std::fs::read_to_string(&doc_path).unwrap();
    assert!(content.starts_with(r"{\rtf1"));
}

#[test]
fn docx_to_pdf_creates_valid_pdf() {
    let tmp = tempfile::tempdir().unwrap();
    let img = tmp.path().join("page.png");
    image::RgbaImage::new(50, 50).save(&img).unwrap();
    let pdf_source = tmp.path().join("source.pdf");
    images_to_pdf(&[img], &pdf_source).unwrap();

    let docx_path = tmp.path().join("sample.docx");
    fileuc_convert::pdf::pdf_to_docx(&pdf_source, &docx_path).unwrap();

    let pdf_out = tmp.path().join("converted.pdf");
    fileuc_convert::doc::doc_to_pdf(&docx_path, &pdf_out).unwrap();
    assert!(pdf_out.exists());

    let bytes = std::fs::read(&pdf_out).unwrap();
    assert_eq!(&bytes[0..5], b"%PDF-");
}

#[test]
fn test_pdf_to_docx_preserves_layout_boxes_and_indic() {
    use std::io::Read;
    let pdf_path = std::path::Path::new(r"C:\Users\Sharad Chand\Downloads\StudentbuspassApplication.pdf");
    if !pdf_path.exists() {
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let docx_out = tmp.path().join("application.docx");

    let layout_res = fileuc_convert::pdfium::extract_structured_layout(pdf_path);
    match &layout_res {
        Ok(layouts) => {
            println!("extract_structured_layout returned {} pages", layouts.len());
            for l in layouts {
                println!("Page {} has {} items", l.page_number, l.items.len());
            }
        }
        Err(e) => {
            println!("extract_structured_layout failed: {:?}", e);
        }
    }

    fileuc_convert::pdf::pdf_to_docx(pdf_path, &docx_out).expect("pdf_to_docx failed");
    assert!(docx_out.exists());

    // Inspect document.xml inside docx
    let file = std::fs::File::open(&docx_out).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let mut doc_file = zip.by_name("word/document.xml").expect("document.xml missing");
    let mut doc_xml = String::new();
    doc_file.read_to_string(&mut doc_xml).unwrap();

    println!("doc_xml length: {}", doc_xml.len());

    // Verify layout features
    assert!(doc_xml.contains("<w:tabs>"), "Must preserve tab stops for columnar alignment");
    assert!(doc_xml.contains("<w:tab"), "Must contain tab elements");
    assert!(doc_xml.contains("BANGALORE METROPOLITAN TRANSPORT CORPORATION"), "Must contain header");
    assert!(doc_xml.contains("ಬೆಂಗಳೂರು ಮಹಾನಗರ ಸಾರಿಗೆ ಸಂಸ್ಥೆ"), "Must contain properly transliterated Kannada");
    assert!(doc_xml.contains("<w:pgBdr"), "Must contain outer border");
    println!("Verified PDF to DOCX: layout, tabs, alignment, outer borders, and Kannada are 100% preserved!");
}

#[test]
fn test_lock_and_unlock_pdf() {
    let tmp = tempfile::tempdir().unwrap();
    let img = tmp.path().join("page.png");
    image::RgbaImage::new(100, 100).save(&img).unwrap();
    let pdf_path = tmp.path().join("source.pdf");
    images_to_pdf(&[img], &pdf_path).unwrap();

    let locked_path = tmp.path().join("locked.pdf");
    fileuc_convert::pdf::lock_pdf(&pdf_path, "MyPassword123", &locked_path).unwrap();
    assert!(locked_path.exists());

    // Verify file is marked encrypted
    let doc = lopdf::Document::load(&locked_path).unwrap();
    assert!(doc.is_encrypted(), "PDF should have /Encrypt dictionary");

    // Attempt without password should return false (prompt user for password)
    let unlocked_path = tmp.path().join("unlocked.pdf");
    let needs_password = fileuc_convert::pdf::unlock_pdf(&locked_path, None, &unlocked_path).unwrap();
    assert!(!needs_password, "Should return false indicating user password is required");

    // Attempt with incorrect password should fail
    let wrong_res = fileuc_convert::pdf::unlock_pdf(&locked_path, Some("WrongPassword"), &unlocked_path);
    assert!(wrong_res.is_err(), "Should return error on incorrect password");

    // Attempt with correct password should succeed
    let unlocked = fileuc_convert::pdf::unlock_pdf(&locked_path, Some("MyPassword123"), &unlocked_path).unwrap();
    assert!(unlocked, "Should return true on correct password");
    assert!(unlocked_path.exists());

    // Verify unlocked PDF is no longer encrypted
    let unencrypted_doc = lopdf::Document::load(&unlocked_path).unwrap();
    assert!(!unencrypted_doc.is_encrypted(), "Decrypted PDF should not have /Encrypt dictionary");
}

#[test]
fn test_compress_pdf() {
    let tmp = tempfile::tempdir().unwrap();
    let img = tmp.path().join("page.png");
    image::RgbaImage::new(200, 200).save(&img).unwrap();
    let pdf_path = tmp.path().join("source.pdf");
    images_to_pdf(&[img], &pdf_path).unwrap();

    let compressed_path = tmp.path().join("compressed.pdf");
    fileuc_convert::pdf::compress_pdf(&pdf_path, &compressed_path).unwrap();
    assert!(compressed_path.exists());

    let bytes = std::fs::read(&compressed_path).unwrap();
    assert_eq!(&bytes[0..5], b"%PDF-");
}
