use fileuc_types::{ConvertError, ImageFormat};
use image::ImageDecoder;
use std::path::{Path, PathBuf};

/// Reads an image from disk, inspecting EXIF orientation metadata tags and physically
/// applying rotation/flip transformations to the raw pixel buffer before processing.
/// This prevents converted images (WebP, PNG, JPEG, PDF) from appearing sideways or upside down.
pub fn open_image_with_orientation(src: &Path) -> Result<image::DynamicImage, ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "heic" || ext == "heif" {
        return crate::heic::decode_heic(src);
    }

    let file = match std::fs::File::open(src) {
        Ok(f) => f,
        Err(io_err) => {
            if io_err.kind() == std::io::ErrorKind::PermissionDenied {
                return Err(ConvertError::SourceLocked(src.to_string_lossy().to_string()));
            }
            return Err(ConvertError::IoError(io_err.to_string()));
        }
    };
    let buf_reader = std::io::BufReader::new(file);

    let reader = match image::ImageReader::new(buf_reader).with_guessed_format() {
        Ok(r) => r,
        Err(_) => {
            return image::open(src)
                .map_err(|e| ConvertError::ConverterError(format!("Failed to open image: {}", e)));
        }
    };

    match reader.into_decoder() {
        Ok(mut decoder) => {
            let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
            match image::DynamicImage::from_decoder(decoder) {
                Ok(mut img) => {
                    img.apply_orientation(orientation);
                    Ok(img)
                }
                Err(_) => {
                    image::open(src)
                        .map_err(|e| ConvertError::ConverterError(format!("Failed to open image: {}", e)))
                }
            }
        }
        Err(_) => {
            image::open(src)
                .map_err(|e| ConvertError::ConverterError(format!("Failed to open image: {}", e)))
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ImageOptions {
    pub quality: Option<u8>,
    pub resize: Option<ResizeSpec>,
    pub grayscale: bool,
    pub custom_ext: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
pub struct ResizeSpec {
    pub width: u32,
    pub height: u32,
}

pub fn convert_image(
    src: &Path,
    format: ImageFormat,
    opts: &ImageOptions,
) -> Result<PathBuf, ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "svg" {
        return crate::svg::render_svg(src, format, opts.resize.map(|r| r.width));
    }

    let mut img = open_image_with_orientation(src)?;

    if opts.grayscale {
        img = img.grayscale();
    }

    if let Some(resize) = opts.resize {
        img = img.resize_exact(
            resize.width,
            resize.height,
            image::imageops::FilterType::Lanczos3,
        );
    }

    let target_ext = opts.custom_ext.unwrap_or_else(|| match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::WebP => "webp",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Ico => "ico",
        ImageFormat::Tiff => "tiff",
        ImageFormat::Avif => "avif",
    });

    let out_path = crate::output::resolve_output_path(src, target_ext);

    match format {
        ImageFormat::Jpeg => {
            let quality = opts.quality.unwrap_or(90);
            let file = std::fs::File::create(&out_path)
                .map_err(|e| ConvertError::IoError(e.to_string()))?;
            let mut writer = std::io::BufWriter::new(file);
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality);

            // If the image has transparency, composite onto clean white background
            // so transparent areas don't turn into black boxes
            if img.color().has_alpha() {
                let rgba = img.to_rgba8();
                let mut rgb = image::RgbImage::new(rgba.width(), rgba.height());
                for (x, y, pixel) in rgba.enumerate_pixels() {
                    let a = pixel[3] as f32 / 255.0;
                    let r = ((pixel[0] as f32 * a) + 255.0 * (1.0 - a)).round() as u8;
                    let g = ((pixel[1] as f32 * a) + 255.0 * (1.0 - a)).round() as u8;
                    let b = ((pixel[2] as f32 * a) + 255.0 * (1.0 - a)).round() as u8;
                    rgb.put_pixel(x, y, image::Rgb([r, g, b]));
                }
                encoder
                    .encode(&rgb, rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
                    .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            } else {
                let rgb = img.to_rgb8();
                encoder
                    .encode(&rgb, rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
                    .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            }
        }
        ImageFormat::Ico => {
            create_authentic_ico(&img, &out_path)?;
        }
        _ => {
            img.save(&out_path)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
        }
    }

    Ok(out_path)
}

pub fn image_to_svg(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // Fast-path: for directly embeddable web formats, get dimensions without full bitmap decoding
    let (width, height, mime, raw_bytes) = match ext.as_str() {
        "jpg" | "jpeg" => {
            let (w, h) = image::image_dimensions(src).map_err(|e| {
                if let Err(io_err) = std::fs::File::open(src) {
                    if io_err.kind() == std::io::ErrorKind::PermissionDenied {
                        return ConvertError::SourceLocked(src.to_string_lossy().to_string());
                    }
                }
                ConvertError::ConverterError(format!("Failed to read image dimensions: {}", e))
            })?;
            let b = std::fs::read(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
            (w, h, "image/jpeg", b)
        }
        "png" => {
            let (w, h) = image::image_dimensions(src).map_err(|e| {
                if let Err(io_err) = std::fs::File::open(src) {
                    if io_err.kind() == std::io::ErrorKind::PermissionDenied {
                        return ConvertError::SourceLocked(src.to_string_lossy().to_string());
                    }
                }
                ConvertError::ConverterError(format!("Failed to read image dimensions: {}", e))
            })?;
            let b = std::fs::read(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
            (w, h, "image/png", b)
        }
        "webp" => {
            let (w, h) = image::image_dimensions(src).map_err(|e| {
                if let Err(io_err) = std::fs::File::open(src) {
                    if io_err.kind() == std::io::ErrorKind::PermissionDenied {
                        return ConvertError::SourceLocked(src.to_string_lossy().to_string());
                    }
                }
                ConvertError::ConverterError(format!("Failed to read image dimensions: {}", e))
            })?;
            let b = std::fs::read(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
            (w, h, "image/webp", b)
        }
        _ => {
            let img = open_image_with_orientation(src)?;
            let (w, h) = (img.width(), img.height());
            let mut buf = std::io::Cursor::new(Vec::new());
            img.write_to(&mut buf, image::ImageFormat::Png)
                .map_err(|e| ConvertError::ConverterError(format!("Failed to encode PNG for SVG: {}", e)))?;
            (w, h, "image/png", buf.into_inner())
        }
    };

    use base64::prelude::*;
    use std::io::Write;

    let guard = crate::output_guard::OutputGuard::new(output);
    let out_file = std::fs::File::create(output).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut writer = std::io::BufWriter::new(out_file);

    write!(
        writer,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{}" height="{}" viewBox="0 0 {} {}">
  <image width="{}" height="{}" xlink:href="data:{};base64,"#,
        width, height, width, height, width, height, mime
    )
    .map_err(|e| ConvertError::IoError(e.to_string()))?;

    {
        use base64::write::EncoderWriter;
        let mut encoder = EncoderWriter::new(&mut writer, &BASE64_STANDARD);
        encoder
            .write_all(&raw_bytes)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
        encoder
            .finish()
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
    }

    write!(writer, "\"/>\n</svg>").map_err(|e| ConvertError::IoError(e.to_string()))?;
    writer.flush().map_err(|e| ConvertError::IoError(e.to_string()))?;

    guard.disarm();
    Ok(())
}

/// Helper to fit any image into a square canvas with transparent padding, preserving aspect ratio.
pub fn fit_to_square(img: &image::DynamicImage) -> image::DynamicImage {
    if img.width() == img.height() {
        return img.clone();
    }
    let max_dim = img.width().max(img.height());
    let mut canvas = image::RgbaImage::new(max_dim, max_dim);
    let offset_x = (max_dim - img.width()) / 2;
    let offset_y = (max_dim - img.height()) / 2;
    image::imageops::overlay(&mut canvas, &img.to_rgba8(), offset_x as i64, offset_y as i64);
    image::DynamicImage::ImageRgba8(canvas)
}

/// Generates an authentic multi-resolution Windows .ico file containing standard resolutions:
/// 16x16, 24x24, 32x32, 48x48, 64x64, 128x128, and 256x256.
/// Encodes an image into standard Windows ICO DIB format (BITMAPINFOHEADER + 32-bit BGRA + 1-bit AND mask).
/// Fully compatible with Windows Photos, Paint, GDI+, WIC, Explorer thumbnails, and legacy viewers.
fn encode_ico_dib(img: &image::RgbaImage) -> Vec<u8> {
    let width = img.width();
    let height = img.height();

    // BITMAPINFOHEADER is 40 bytes.
    // In ICO, biHeight is doubled (2 * height) for XOR mask + AND mask.
    let bi_size: u32 = 40;
    let bi_width: i32 = width as i32;
    let bi_height: i32 = (height * 2) as i32;
    let bi_planes: u16 = 1;
    let bi_bit_count: u16 = 32;
    let bi_compression: u32 = 0; // BI_RGB

    let xor_bytes_len = (width * height * 4) as usize;
    // AND mask row stride is padded to 32 bits (4 bytes)
    let and_row_stride = ((width + 31) / 32 * 4) as usize;
    let and_bytes_len = and_row_stride * (height as usize);
    let bi_size_image: u32 = (xor_bytes_len + and_bytes_len) as u32;

    let mut buf = Vec::with_capacity(40 + xor_bytes_len + and_bytes_len);
    buf.extend_from_slice(&bi_size.to_le_bytes());
    buf.extend_from_slice(&bi_width.to_le_bytes());
    buf.extend_from_slice(&bi_height.to_le_bytes());
    buf.extend_from_slice(&bi_planes.to_le_bytes());
    buf.extend_from_slice(&bi_bit_count.to_le_bytes());
    buf.extend_from_slice(&bi_compression.to_le_bytes());
    buf.extend_from_slice(&bi_size_image.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // biXPelsPerMeter
    buf.extend_from_slice(&0u32.to_le_bytes()); // biYPelsPerMeter
    buf.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    buf.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

    // XOR mask: 32-bit BGRA in bottom-up row order
    for y in (0..height).rev() {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            // pixel is [R, G, B, A] -> store B, G, R, A
            buf.push(pixel[2]);
            buf.push(pixel[1]);
            buf.push(pixel[0]);
            buf.push(pixel[3]);
        }
    }

    // AND mask: 1 bit per pixel, bottom-up row order, padded to 4 bytes per row
    for y in (0..height).rev() {
        let mut row_bytes = vec![0u8; and_row_stride];
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            if pixel[3] == 0 {
                let byte_idx = (x / 8) as usize;
                let bit_idx = 7 - (x % 8);
                row_bytes[byte_idx] |= 1 << bit_idx;
            }
        }
        buf.extend_from_slice(&row_bytes);
    }

    buf
}

/// Creates a multi-resolution ICO file (256, 128, 64, 48, 32, 24, 16).
/// Encodes frames using canonical 32-bit DIB format so Windows Photos, GDI+, WIC,
/// Paint, and Windows Explorer can open and render the file with zero errors.
pub fn create_authentic_ico(img: &image::DynamicImage, output: &Path) -> Result<(), ConvertError> {
    const SIZES: &[u32] = &[256, 128, 64, 48, 32, 24, 16];

    let square_img = fit_to_square(img);

    struct IcoEntry {
        size: u32,
        data: Vec<u8>,
    }

    let mut entries = Vec::with_capacity(SIZES.len());
    for &size in SIZES {
        let resized = square_img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        let rgba = resized.to_rgba8();
        let dib_bytes = encode_ico_dib(&rgba);
        entries.push(IcoEntry {
            size,
            data: dib_bytes,
        });
    }

    let num_images = entries.len() as u16;
    let header_size = 6 + (entries.len() * 16);
    let mut current_offset = header_size as u32;

    let mut ico_bytes = Vec::with_capacity(header_size + entries.iter().map(|e| e.data.len()).sum::<usize>());

    // 1. ICONDIR Header (6 bytes)
    ico_bytes.extend_from_slice(&0u16.to_le_bytes()); // Reserved (must be 0)
    ico_bytes.extend_from_slice(&1u16.to_le_bytes()); // Resource type (1 for ICO)
    ico_bytes.extend_from_slice(&num_images.to_le_bytes()); // Number of images

    // 2. ICONDIRENTRY Records (16 bytes per image)
    for entry in &entries {
        let dim_byte = if entry.size >= 256 { 0u8 } else { entry.size as u8 };
        ico_bytes.push(dim_byte); // Width: 0 means 256px
        ico_bytes.push(dim_byte); // Height: 0 means 256px
        ico_bytes.push(0);        // Color count (0 for 32-bit RGBA)
        ico_bytes.push(0);        // Reserved
        ico_bytes.extend_from_slice(&1u16.to_le_bytes());  // Color planes (1)
        ico_bytes.extend_from_slice(&32u16.to_le_bytes()); // Bits per pixel (32)
        ico_bytes.extend_from_slice(&(entry.data.len() as u32).to_le_bytes()); // Size of image data
        ico_bytes.extend_from_slice(&current_offset.to_le_bytes()); // Offset to image data
        current_offset += entry.data.len() as u32;
    }

    // 3. Image Payloads (DIB data chunks)
    for entry in &entries {
        ico_bytes.extend_from_slice(&entry.data);
    }

    if let Some(parent) = output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    std::fs::write(output, ico_bytes).map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}

/// Generates a comprehensive modern App/Web Icon Pack inside `output_dir`.
/// Outputs:
/// - favicon.ico (multi-resolution 16, 24, 32, 48, 64, 128, 256)
/// - favicon-16x16.png
/// - favicon-24x24.png
/// - favicon-32x32.png
/// - favicon-48x48.png
/// - icon-64x64.png
/// - icon-128x128.png
/// - icon-256x256.png
/// - apple-touch-icon.png (180x180)
/// - icon-192x192.png
/// - icon-512x512.png
pub fn create_icon_pack(src: &Path, output_dir: &Path) -> Result<Vec<PathBuf>, ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    std::fs::create_dir_all(output_dir).map_err(|e| ConvertError::IoError(e.to_string()))?;

    let img = open_image_with_orientation(src)?;
    let square_img = fit_to_square(&img);

    let mut generated = Vec::new();

    // 1. Master multi-res favicon.ico
    let ico_path = output_dir.join("favicon.ico");
    create_authentic_ico(&square_img, &ico_path)?;
    generated.push(ico_path);

    // 2. Individual PNG assets
    const PNG_SIZES: &[(&str, u32)] = &[
        ("favicon-16x16.png", 16),
        ("favicon-24x24.png", 24),
        ("favicon-32x32.png", 32),
        ("favicon-48x48.png", 48),
        ("icon-64x64.png", 64),
        ("icon-128x128.png", 128),
        ("icon-256x256.png", 256),
        ("apple-touch-icon.png", 180),
        ("icon-192x192.png", 192),
        ("icon-512x512.png", 512),
    ];

    for &(filename, size) in PNG_SIZES {
        let resized = square_img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        let file_path = output_dir.join(filename);
        resized
            .save(&file_path)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to save {}: {}", filename, e)))?;
        generated.push(file_path);
    }

    Ok(generated)
}

/// Converts raw Windows CF_DIB / CF_DIBV5 clipboard bytes into a standard PNG file on disk.
pub fn save_dib_as_png(dib_bytes: &[u8], out_path: &Path) -> Result<(), ConvertError> {
    if dib_bytes.len() < 40 {
        return Err(ConvertError::ConverterError("DIB data too short".to_string()));
    }

    let bi_size = u32::from_le_bytes([dib_bytes[0], dib_bytes[1], dib_bytes[2], dib_bytes[3]]);
    let bi_bit_count = if dib_bytes.len() >= 16 {
        u16::from_le_bytes([dib_bytes[14], dib_bytes[15]])
    } else {
        24
    };
    let bi_clr_used = if dib_bytes.len() >= 36 {
        u32::from_le_bytes([dib_bytes[32], dib_bytes[33], dib_bytes[34], dib_bytes[35]])
    } else {
        0
    };
    let palette_colors = if bi_clr_used > 0 {
        bi_clr_used
    } else if bi_bit_count <= 8 {
        1 << bi_bit_count
    } else {
        0
    };
    let bi_compression = if dib_bytes.len() >= 20 {
        u32::from_le_bytes([dib_bytes[16], dib_bytes[17], dib_bytes[18], dib_bytes[19]])
    } else {
        0
    };
    let masks_size = if bi_size == 40 && bi_compression == 3 { 12 } else { 0 };

    let off_bits = 14 + bi_size + masks_size + palette_colors * 4;
    let file_size = (14 + dib_bytes.len()) as u32;

    let mut bmp = Vec::with_capacity(14 + dib_bytes.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size.to_le_bytes());
    bmp.extend_from_slice(&0u16.to_le_bytes());
    bmp.extend_from_slice(&0u16.to_le_bytes());
    bmp.extend_from_slice(&off_bits.to_le_bytes());
    bmp.extend_from_slice(dib_bytes);

    let img = image::load_from_memory(&bmp)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to decode DIB as BMP: {}", e)))?;

    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    img.save(out_path)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to save clipboard image as PNG: {}", e)))
}
