use fileuc_convert::pptx::pptx_to_pdf;
use std::io::Write;

#[test]
fn test_pptx_to_pdf_conversion() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let pptx_path = temp_dir.path().join("sample.pptx");
    let pdf_path = temp_dir.path().join("sample.pdf");

    // 1. Create a synthetic PPTX archive with a slide and image
    let file = std::fs::File::create(&pptx_path).expect("Failed to create pptx");
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    zip.start_file("ppt/presentation.xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>"#).unwrap();

    zip.start_file("ppt/slides/slide1.xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:txBody>
          <a:p><a:r><a:t>Sample Slide Title</a:t></a:r></a:p>
          <a:p><a:r><a:t>High fidelity presentation slide text</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#).unwrap();

    // Embed test image
    let mut img_bytes = Vec::new();
    let img = image::RgbImage::new(60, 60);
    img.write_to(&mut std::io::Cursor::new(&mut img_bytes), image::ImageFormat::Png).unwrap();
    zip.start_file("ppt/media/image1.png", options).unwrap();
    zip.write_all(&img_bytes).unwrap();

    // Rels pointing to media
    zip.start_file("ppt/slides/_rels/slide1.xml.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>
</Relationships>"#).unwrap();

    zip.finish().unwrap();

    // 2. Convert PPTX -> PDF
    pptx_to_pdf(&pptx_path, &pdf_path).expect("pptx_to_pdf failed");
    assert!(pdf_path.exists());
    let pdf_bytes = std::fs::read(&pdf_path).unwrap();
    assert!(pdf_bytes.len() > 100);
    assert_eq!(&pdf_bytes[0..5], b"%PDF-");
}
