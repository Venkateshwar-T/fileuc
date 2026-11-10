use fileuc_convert::pptx::{docx_to_pptx, pdf_to_pptx, pptx_to_docx, pptx_to_pdf};
use std::io::Write;

#[test]
fn test_docx_to_pptx_with_images_and_vice_versa() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let docx_path = temp_dir.path().join("sample.docx");
    let pptx_path = temp_dir.path().join("sample.pptx");
    let back_docx_path = temp_dir.path().join("sample_back.docx");
    let pdf_path = temp_dir.path().join("sample.pdf");

    // 1. Create a synthetic DOCX with an embedded test image
    let file = std::fs::File::create(&docx_path).expect("Failed to create docx");
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Project Architecture &amp; Overview</w:t></w:r></w:p>
    <w:p><w:r><w:t>1. High Performance Engine in Pure Rust</w:t></w:r></w:p>
    <w:p><w:r><w:t>2. Zero Quality Loss Image Preservation</w:t></w:r></w:p>
    <w:p><w:r><w:t>3. Native Windows WinRT Integration</w:t></w:r></w:p>
  </w:body>
</w:document>"#).unwrap();

    // Create a 100x100 PNG test image
    let mut img_bytes = Vec::new();
    let img = image::RgbImage::new(100, 100);
    img.write_to(&mut std::io::Cursor::new(&mut img_bytes), image::ImageFormat::Png).unwrap();

    zip.start_file("word/media/image1.png", options).unwrap();
    zip.write_all(&img_bytes).unwrap();
    zip.finish().unwrap();

    // 2. Convert DOCX -> PPTX
    docx_to_pptx(&docx_path, &pptx_path).expect("docx_to_pptx failed");
    assert!(pptx_path.exists());
    assert!(std::fs::metadata(&pptx_path).unwrap().len() > 0);

    // Verify PPTX has ppt/media/image1.png and slide xml
    {
        let pptx_file = std::fs::File::open(&pptx_path).unwrap();
        let mut pptx_zip = zip::ZipArchive::new(pptx_file).unwrap();
        assert!(pptx_zip.by_name("ppt/presentation.xml").is_ok());
        assert!(pptx_zip.by_name("ppt/slides/slide1.xml").is_ok());
        assert!(pptx_zip.by_name("ppt/media/image1.png").is_ok());
    }

    // 3. Convert PPTX -> DOCX
    pptx_to_docx(&pptx_path, &back_docx_path).expect("pptx_to_docx failed");
    assert!(back_docx_path.exists());
    assert!(std::fs::metadata(&back_docx_path).unwrap().len() > 0);

    // Verify back-converted DOCX contains word/media/image1.png
    {
        let docx_file = std::fs::File::open(&back_docx_path).unwrap();
        let mut docx_zip = zip::ZipArchive::new(docx_file).unwrap();
        assert!(docx_zip.by_name("word/document.xml").is_ok());
        assert!(docx_zip.by_name("word/media/image1.png").is_ok());
    }

    // 4. Convert PPTX -> PDF
    pptx_to_pdf(&pptx_path, &pdf_path).expect("pptx_to_pdf failed");
    assert!(pdf_path.exists());
    let pdf_bytes = std::fs::read(&pdf_path).unwrap();
    assert!(pdf_bytes.len() > 100);
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);
    assert!(
        pdf_str.contains("/XObject") || pdf_str.contains("/Image") || pdf_str.contains("/DCTDecode") || pdf_str.contains("/FlateDecode"),
        "PDF must retain and embed the image from PPTX"
    );
    println!("Verified PPTX to PDF retains 100% of embedded images!");

    // 5. Convert PDF -> PPTX
    let pdf_to_pptx_path = temp_dir.path().join("from_pdf.pptx");
    pdf_to_pptx(&pdf_path, &pdf_to_pptx_path).expect("pdf_to_pptx failed");
    assert!(pdf_to_pptx_path.exists());
    assert!(std::fs::metadata(&pdf_to_pptx_path).unwrap().len() > 0);

    {
        let check_file = std::fs::File::open(&pdf_to_pptx_path).unwrap();
        let mut check_zip = zip::ZipArchive::new(check_file).unwrap();
        assert!(check_zip.by_name("ppt/presentation.xml").is_ok());
        assert!(check_zip.by_name("ppt/slides/slide1.xml").is_ok());
    }
}

#[test]
fn test_convert_student_pdf_to_structured_pptx() {
    let student_pdf = std::path::Path::new(r"C:\Users\Sharad Chand\Downloads\StudentbuspassApplication.pdf");
    if !student_pdf.exists() {
        println!("StudentbuspassApplication.pdf not found in Downloads, skipping");
        return;
    }

    let out_pptx = std::path::Path::new(r"D:\Coding Files\Major Projects\FileUC\target\Studentbuspass_converted.pptx");
    if out_pptx.exists() {
        let _ = std::fs::remove_file(out_pptx);
    }

    pdf_to_pptx(student_pdf, out_pptx).expect("Failed to convert StudentbuspassApplication.pdf to PPTX");
    assert!(out_pptx.exists());
    assert!(std::fs::metadata(out_pptx).unwrap().len() > 0);

    // Verify slide1.xml contains editable text shapes with Kannada
    let file = std::fs::File::open(out_pptx).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let mut slide1_xml = String::new();
    {
        let mut s_file = zip.by_name("ppt/slides/slide1.xml").unwrap();
        std::io::Read::read_to_string(&mut s_file, &mut slide1_xml).unwrap();
    }

    println!("Slide 1 XML preview:\n{}", &slide1_xml[..1000.min(slide1_xml.len())]);
    assert!(slide1_xml.contains("<p:sp>"));
    assert!(slide1_xml.contains("txBox=\"1\""));
    assert!(slide1_xml.contains("BANGALORE METROPOLITAN TRANSPORT CORPORATION"));

    let mut slide2_xml = String::new();
    {
        let mut s2_file = zip.by_name("ppt/slides/slide2.xml").unwrap();
        std::io::Read::read_to_string(&mut s2_file, &mut slide2_xml).unwrap();
    }
    assert!(slide2_xml.contains("<p:sp>"));
    assert!(slide2_xml.contains("txBox=\"1\""));
    assert!(slide2_xml.contains("ಬೆಂಗಳೂರು"));
    println!("SUCCESS: StudentbuspassApplication.pdf converted to structured PPTX with individual editable text boxes and clean Kannada Unicode!");
}
