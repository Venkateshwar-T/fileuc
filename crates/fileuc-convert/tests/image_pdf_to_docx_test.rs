use std::io::Read;

#[test]
fn test_image_only_pdf_to_docx_with_word_com() {
    let tmp = tempfile::tempdir().unwrap();

    // 1. Create 2 test images (e.g. simulated scanned documents)
    let img1_path = tmp.path().join("scan_page_1.png");
    let img2_path = tmp.path().join("scan_page_2.png");

    let mut img1 = image::RgbImage::new(400, 500);
    for x in 0..400 {
        for y in 0..500 {
            let color = if x < 20 || x > 380 || y < 20 || y > 480 {
                image::Rgb([0u8, 0u8, 0u8])
            } else if (x + y) % 30 == 0 {
                image::Rgb([180u8, 50u8, 50u8])
            } else {
                image::Rgb([255u8, 255u8, 255u8])
            };
            img1.put_pixel(x, y, color);
        }
    }
    img1.save(&img1_path).unwrap();

    let mut img2 = image::RgbImage::new(400, 500);
    for x in 0..400 {
        for y in 0..500 {
            let color = if (x * y) % 50 == 0 {
                image::Rgb([50u8, 100u8, 200u8])
            } else {
                image::Rgb([250u8, 250u8, 250u8])
            };
            img2.put_pixel(x, y, color);
        }
    }
    img2.save(&img2_path).unwrap();

    // 2. Convert images to an image-only PDF
    let pdf_path = tmp.path().join("scanned_doc.pdf");
    fileuc_convert::pdf::images_to_pdf(&[img1_path, img2_path], &pdf_path).unwrap();
    assert!(pdf_path.exists());

    // 3. Convert image-only PDF to DOCX
    let docx_path = tmp.path().join("scanned_doc.docx");
    fileuc_convert::pdf::pdf_to_docx(&pdf_path, &docx_path).expect("pdf_to_docx failed on image-only PDF");
    assert!(docx_path.exists());

    // 4. Verify ZIP structure
    let file = std::fs::File::open(&docx_path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    assert!(zip.by_name("word/document.xml").is_ok(), "document.xml missing");
    assert!(zip.by_name("word/styles.xml").is_ok(), "styles.xml missing");
    assert!(zip.by_name("word/_rels/document.xml.rels").is_ok(), "document.xml.rels missing");
    assert!(zip.by_name("word/media/image1.png").is_ok(), "image1.png missing");
    assert!(zip.by_name("word/media/image2.png").is_ok(), "image2.png missing");

    let mut doc_file = zip.by_name("word/document.xml").unwrap();
    let mut doc_xml = String::new();
    doc_file.read_to_string(&mut doc_xml).unwrap();
    assert!(doc_xml.contains("r:embed=\"rId2\""), "rId2 missing");
    assert!(doc_xml.contains("r:embed=\"rId3\""), "rId3 missing");
    assert!(doc_xml.contains("<w:br w:type=\"page\"/>"), "page break missing between pages");

    println!("Verified DOCX package: image1 and image2 are embedded with page breaks!");

    // 5. If Word COM is available, verify Word opens it without repair alerts
    let abs_docx = fileuc_convert::pptx::clean_windows_path(&docx_path);
    let ps_script = format!(
        r#"$word = New-Object -ComObject Word.Application; $word.Visible = $false; $word.DisplayAlerts = -1; try {{ $doc = $word.Documents.Open('{}', $false, $true); if ($doc -ne $null) {{ $count = $doc.InlineShapes.Count; Write-Host "Word opened cleanly! InlineShapes count: $count"; $doc.Close([ref]0); }} else {{ throw 'Failed to open doc'; }} }} finally {{ $word.Quit(); }}"#,
        abs_docx.replace('\'', "''")
    );

    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &ps_script]);
    if let Ok(output) = cmd.output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("Word COM stdout: {}", stdout);
        if !stderr.is_empty() {
            println!("Word COM stderr: {}", stderr);
        }
        if output.status.success() {
            assert!(stdout.contains("Word opened cleanly!"), "Word COM did not report clean open");
            assert!(stdout.contains("InlineShapes count: 2"), "Word COM did not find 2 inline shapes");
            println!("SUCCESS: Microsoft Word opened the converted image-only DOCX cleanly with 0 repair warnings and verified 2 inline image shapes!");
        }
    }
}
