use fileuc_convert::size_reducer::{reduce_size, reduced_output_path};
use std::path::PathBuf;

#[test]
fn test_reduced_output_path() {
    let p = PathBuf::from(r"C:\docs\presentation.pdf");
    let out = reduced_output_path(&p);
    assert_eq!(out, PathBuf::from(r"C:\docs\presentation_reduced.pdf"));

    let img = PathBuf::from("photo.jpg");
    assert_eq!(reduced_output_path(&img), PathBuf::from("photo_reduced.jpg"));
}

#[test]
fn test_cannot_target_larger_or_equal_size() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("test.jpg");

    // Create a 100x100 test image (~10KB uncompressed)
    let img = image::RgbImage::new(100, 100);
    img.save(&src).unwrap();

    let orig_size = src.metadata().unwrap().len();
    assert!(orig_size > 0);

    // Target size >= original size must be rejected immediately!
    let res = reduce_size(&src, None, orig_size + 1000);
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("must be smaller than current file size"));
}

#[test]
fn test_reduce_image_size_reduces_successfully() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("large.jpg");

    // Create a colorful 800x800 image
    let mut img = image::RgbImage::new(800, 800);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = image::Rgb([(x % 255) as u8, (y % 255) as u8, ((x + y) % 255) as u8]);
    }
    img.save(&src).unwrap();

    let orig_size = src.metadata().unwrap().len();
    assert!(orig_size > 20_000);

    // Request reduction to 15KB
    let target = 15 * 1024;
    let out = reduce_size(&src, None, target).unwrap();

    assert!(out.exists());
    let new_size = out.metadata().unwrap().len();
    // Must be strictly smaller than original!
    assert!(new_size < orig_size);
    // Must be within or close to target
    assert!(new_size <= target);
}

#[test]
fn test_real_user_pdfs() {
    let p1 = PathBuf::from(r"E:\Important Files\Venkateshwar\StudentPortfolio.pdf");
    if p1.exists() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("StudentPortfolio_reduced.pdf");
        println!("Testing StudentPortfolio.pdf (orig: {} bytes)", p1.metadata().unwrap().len());
        let res200 = reduce_size(&p1, Some(&dest), 200 * 1024);
        println!("StudentPortfolio 200KB result: {:?}", res200);
        if let Ok(ref out) = res200 {
            let out_len = out.metadata().unwrap().len();
            println!("Reduced 200KB size: {} bytes (target: {} bytes)", out_len, 200 * 1024);
            assert!(out_len <= 200 * 1024);

            // Verify integrity: must be a valid PDF parseable by lopdf
            let lopdf_doc = lopdf::Document::load(out).expect("Reduced PDF must be valid lopdf document");
            assert!(lopdf_doc.get_pages().len() > 0);

            // Verify integrity: must be valid and renderable by pdfium
            let pdfium = fileuc_convert::pdfium::init_pdfium().expect("Pdfium should initialize");
            let pdoc = pdfium.load_pdf_from_file(out, None).expect("Pdfium should open reduced PDF");
            assert!(pdoc.pages().len() > 0);
            println!("Verified StudentPortfolio reduced PDF integrity: {} pages intact!", pdoc.pages().len());
        }
        let dest300 = tmp.path().join("StudentPortfolio_300.pdf");
        let res300 = reduce_size(&p1, Some(&dest300), 300 * 1024);
        println!("StudentPortfolio 300KB result: {:?}", res300);
        if let Ok(ref out) = res300 {
            let out_len = out.metadata().unwrap().len();
            println!("Reduced 300KB size: {} bytes", out_len);
            assert!(out_len <= 300 * 1024);
            let lopdf_doc = lopdf::Document::load(out).expect("Reduced PDF must be valid lopdf document");
            assert!(lopdf_doc.get_pages().len() > 0);
        }
    }

    let p2 = PathBuf::from(r"E:\Important Files\Venkateshwar\vtu 2026.pdf");
    if p2.exists() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("vtu_reduced.pdf");
        println!("Testing vtu 2026.pdf (orig: {} bytes)", p2.metadata().unwrap().len());
        let res = reduce_size(&p2, Some(&dest), 200 * 1024);
        println!("vtu 200KB result: {:?}", res);
        if let Ok(ref out) = res {
            let out_len = out.metadata().unwrap().len();
            println!("Reduced size: {} bytes (target: {} bytes)", out_len, 200 * 1024);
            assert!(out_len <= 200 * 1024);

            // Verify integrity
            let lopdf_doc = lopdf::Document::load(out).expect("Reduced PDF must be valid lopdf document");
            assert!(lopdf_doc.get_pages().len() > 0);

            let pdfium = fileuc_convert::pdfium::init_pdfium().expect("Pdfium should initialize");
            let pdoc = pdfium.load_pdf_from_file(out, None).expect("Pdfium should open reduced PDF");
            assert!(pdoc.pages().len() > 0);
            println!("Verified vtu 2026 reduced PDF integrity: {} pages intact!", pdoc.pages().len());
        }
    }
}
