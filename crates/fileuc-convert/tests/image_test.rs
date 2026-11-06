use fileuc_convert::image::{convert_image, ImageOptions};
use fileuc_types::ImageFormat;

#[test]
fn png_to_jpg_produces_valid_jpeg() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("test.png");
    image::RgbaImage::new(10, 10).save(&src).unwrap();
    let result = convert_image(&src, ImageFormat::Jpeg, &ImageOptions::default()).unwrap();
    assert!(result.exists());
    assert_eq!(result.extension().unwrap(), "jpg");
    // Verify it's actually a JPEG by checking magic bytes
    let bytes = std::fs::read(&result).unwrap();
    assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
}

#[test]
fn convert_locked_file_returns_source_locked() {
    // Review Focus #4: locked files
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("locked.png");
    std::fs::write(&src, b"not a real image").unwrap();
    let result = convert_image(&src, ImageFormat::Jpeg, &ImageOptions::default());
    assert!(result.is_err());
}
