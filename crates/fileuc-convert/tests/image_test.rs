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

#[test]
fn png_to_jpeg_produces_valid_jpeg_with_extension() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("test.png");
    image::RgbaImage::new(10, 10).save(&src).unwrap();
    let opts = ImageOptions {
        custom_ext: Some("jpeg"),
        ..Default::default()
    };
    let result = convert_image(&src, ImageFormat::Jpeg, &opts).unwrap();
    assert!(result.exists());
    assert_eq!(result.extension().unwrap(), "jpeg");
    let bytes = std::fs::read(&result).unwrap();
    assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
}

#[test]
fn image_to_svg_produces_valid_svg_file() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("test.png");
    image::RgbaImage::new(16, 16).save(&src).unwrap();
    let dst = tmp.path().join("test.svg");
    fileuc_convert::image::image_to_svg(&src, &dst).unwrap();
    assert!(dst.exists());
    let content = std::fs::read_to_string(&dst).unwrap();
    assert!(content.contains("<svg"));
    assert!(content.contains("<image"));
    assert!(content.contains("data:image/png;base64,"));
}

#[test]
fn ico_conversion_produces_authentic_multi_res_icon() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("icon_source.png");
    image::RgbaImage::new(128, 128).save(&src).unwrap();
    let result = convert_image(&src, ImageFormat::Ico, &ImageOptions::default()).unwrap();
    assert!(result.exists());
    assert_eq!(result.extension().unwrap(), "ico");

    let bytes = std::fs::read(&result).unwrap();
    assert!(bytes.len() > 118, "ICO file must contain header and image data");

    // Check ICONDIR header: reserved = 0, type = 1 (ICO), count = 7
    let reserved = u16::from_le_bytes([bytes[0], bytes[1]]);
    let res_type = u16::from_le_bytes([bytes[2], bytes[3]]);
    let count = u16::from_le_bytes([bytes[4], bytes[5]]);
    assert_eq!(reserved, 0);
    assert_eq!(res_type, 1);
    assert_eq!(count, 7, "Must contain all 7 standard Windows resolutions");

    // First entry should be 256px (dim byte = 0)
    assert_eq!(bytes[6], 0, "256px icon width is encoded as 0");
    assert_eq!(bytes[7], 0, "256px icon height is encoded as 0");

    // Last entry should be 16px
    let last_entry_offset = 6 + (6 * 16);
    assert_eq!(bytes[last_entry_offset], 16, "16px icon width");
    assert_eq!(bytes[last_entry_offset + 1], 16, "16px icon height");

    // Verify first payload starts with standard PNG signature (89 50 4E 47 0D 0A 1A 0A)
    let first_img_offset = u32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]) as usize;
    assert_eq!(&bytes[first_img_offset..first_img_offset + 8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
}

#[test]
fn test_open_image_with_orientation_and_conversion() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("photo.png");
    image::RgbaImage::new(40, 20).save(&src).unwrap();

    let img = fileuc_convert::image::open_image_with_orientation(&src).unwrap();
    assert_eq!(img.width(), 40);
    assert_eq!(img.height(), 20);

    let webp_out = fileuc_convert::image::convert_image(&src, ImageFormat::WebP, &ImageOptions::default()).unwrap();
    assert!(webp_out.exists());
    assert_eq!(webp_out.extension().unwrap(), "webp");
}

#[test]
fn create_icon_pack_generates_all_assets() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("logo.png");
    image::RgbaImage::new(128, 128).save(&src).unwrap();
    let out_dir = tmp.path().join("logo_icon_pack");
    let files = fileuc_convert::image::create_icon_pack(&src, &out_dir).unwrap();
    assert_eq!(files.len(), 11);
    assert!(out_dir.join("favicon.ico").exists());
    assert!(out_dir.join("favicon-16x16.png").exists());
    assert!(out_dir.join("favicon-24x24.png").exists());
    assert!(out_dir.join("favicon-32x32.png").exists());
    assert!(out_dir.join("favicon-48x48.png").exists());
    assert!(out_dir.join("icon-64x64.png").exists());
    assert!(out_dir.join("icon-128x128.png").exists());
    assert!(out_dir.join("icon-256x256.png").exists());
    assert!(out_dir.join("apple-touch-icon.png").exists());
    assert!(out_dir.join("icon-192x192.png").exists());
    assert!(out_dir.join("icon-512x512.png").exists());
}
