use fileuc_convert::pdf::images_to_pdf;
use std::io::Write;

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
fn docx_to_pdf_creates_valid_pdf() {
    let tmp = tempfile::tempdir().unwrap();
    let docx_path = tmp.path().join("sample.docx");

    // Create a synthetic DOCX file
    let file = std::fs::File::create(&docx_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello World from DOCX</w:t></w:r></w:p></w:body></w:document>"#,
    ).unwrap();
    zip.finish().unwrap();

    let pdf_out = tmp.path().join("converted.pdf");
    fileuc_convert::doc::doc_to_pdf(&docx_path, &pdf_out).unwrap();
    assert!(pdf_out.exists());

    let bytes = std::fs::read(&pdf_out).unwrap();
    assert_eq!(&bytes[0..5], b"%PDF-");
}

#[test]
fn test_pdf_to_docx_conversion() {
    let tmp = tempfile::tempdir().unwrap();
    let img = tmp.path().join("page.png");
    image::RgbaImage::new(100, 100).save(&img).unwrap();
    let pdf_path = tmp.path().join("source.pdf");
    images_to_pdf(&[img], &pdf_path).unwrap();

    let docx_out = tmp.path().join("output.docx");
    let res = fileuc_convert::doc::pdf_to_docx(&pdf_path, &docx_out);
    if fileuc_convert::doc::find_pdf2docx().is_some() {
        assert!(res.is_ok(), "Conversion should succeed when pdf2docx engine is found: {:?}", res);
        assert!(docx_out.exists(), "Output docx should exist");
        assert!(std::fs::metadata(&docx_out).unwrap().len() > 0, "Output docx should not be empty");
    }
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
fn test_owner_locked_pdf_unlocks_silently() {
    let tmp = tempfile::tempdir().unwrap();
    let img = tmp.path().join("page.png");
    image::RgbaImage::new(50, 50).save(&img).unwrap();
    let pdf_path = tmp.path().join("doc.pdf");
    images_to_pdf(&[img], &pdf_path).unwrap();

    // Lock with empty user password ("") and owner password -> owner/permission lock
    let locked_path = tmp.path().join("owner_locked.pdf");
    fileuc_convert::pdf::lock_pdf_with_owner(&pdf_path, "", "MyOwnerPassword", &locked_path).unwrap();
    assert!(locked_path.exists());

    let doc = lopdf::Document::load(&locked_path).unwrap();
    assert!(doc.is_encrypted());

    // Calling unlock_pdf with None should silently unlock it (matching iLovePDF)
    let unlocked_path = tmp.path().join("silent_unlocked.pdf");
    let res = fileuc_convert::pdf::unlock_pdf(&locked_path, None, &unlocked_path).unwrap();
    assert!(res, "Should return true indicating successful silent unlock");
    assert!(unlocked_path.exists());

    let unencrypted_doc = lopdf::Document::load(&unlocked_path).unwrap();
    assert!(!unencrypted_doc.is_encrypted(), "Trailer should have /Encrypt stripped");
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

#[test]
fn test_merge_pdfs_blocks_password_protected_file() {
    let tmp = tempfile::tempdir().unwrap();
    let img = tmp.path().join("page.png");
    image::RgbaImage::new(100, 100).save(&img).unwrap();

    let pdf1 = tmp.path().join("normal.pdf");
    images_to_pdf(&[img.clone()], &pdf1).unwrap();

    let pdf2_plain = tmp.path().join("plain2.pdf");
    images_to_pdf(&[img], &pdf2_plain).unwrap();

    let pdf2_locked = tmp.path().join("locked.pdf");
    fileuc_convert::pdf::lock_pdf(&pdf2_plain, "SecretKey123", &pdf2_locked).unwrap();

    let merged_out = tmp.path().join("merged.pdf");
    let res = fileuc_convert::pdf::merge_pdfs(&[pdf1, pdf2_locked], &merged_out);

    assert!(res.is_err(), "Merging with a password-protected PDF must fail");
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("Cannot merge") && err_msg.contains("is password-protected"),
        "Error message must clearly state the file is password-protected: {}",
        err_msg
    );
    assert!(!merged_out.exists(), "No corrupted PDF output should be generated");
}

#[test]
fn test_pdf_to_excel_conversion() {
    let tmp = tempfile::tempdir().unwrap();
    let img = tmp.path().join("page.png");
    image::RgbaImage::new(100, 100).save(&img).unwrap();
    let pdf_path = tmp.path().join("sample.pdf");
    images_to_pdf(&[img], &pdf_path).unwrap();

    let excel_out = tmp.path().join("output.xlsx");
    let res = fileuc_convert::excel::pdf_to_excel(&pdf_path, &excel_out);
    if fileuc_convert::doc::find_pdf2docx().is_some() {
        assert!(res.is_ok(), "PDF to Excel should succeed: {:?}", res);
        assert!(excel_out.exists(), "Output xlsx should exist");
        assert!(std::fs::metadata(&excel_out).unwrap().len() > 0);
    }
}
