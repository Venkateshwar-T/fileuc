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
