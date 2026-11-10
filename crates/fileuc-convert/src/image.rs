use fileuc_types::{ConvertError, ImageFormat};
use std::path::{Path, PathBuf};

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

    let mut img = if ext == "heic" || ext == "heif" {
        crate::heic::decode_heic(src)?
    } else {
        match image::open(src) {
            Ok(img) => img,
            Err(e) => {
                // Check if file is locked
                if let Err(io_err) = std::fs::File::open(src) {
                    if io_err.kind() == std::io::ErrorKind::PermissionDenied {
                        return Err(ConvertError::SourceLocked(src.to_string_lossy().to_string()));
                    }
                }
                return Err(ConvertError::ConverterError(format!("Failed to open image: {}", e)));
            }
        }
    };

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
            let img = match image::open(src) {
                Ok(img) => img,
                Err(e) => {
                    if let Err(io_err) = std::fs::File::open(src) {
                        if io_err.kind() == std::io::ErrorKind::PermissionDenied {
                            return Err(ConvertError::SourceLocked(src.to_string_lossy().to_string()));
                        }
                    }
                    return Err(ConvertError::ConverterError(format!("Failed to open image: {}", e)));
                }
            };
            let (w, h) = (img.width(), img.height());
            let mut buf = std::io::Cursor::new(Vec::new());
            img.write_to(&mut buf, image::ImageFormat::Png)
                .map_err(|e| ConvertError::ConverterError(format!("Failed to encode PNG for SVG: {}", e)))?;
            (w, h, "image/png", buf.into_inner())
        }
    };

    use base64::prelude::*;
    let b64 = BASE64_STANDARD.encode(&raw_bytes);
    let svg_content = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{}" height="{}" viewBox="0 0 {} {}">
  <image width="{}" height="{}" xlink:href="data:{};base64,{}"/>
</svg>"#,
        width, height, width, height, width, height, mime, b64
    );

    std::fs::write(output, svg_content.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}

/// Generates an authentic multi-resolution Windows .ico file containing standard resolutions:
/// 16x16, 24x24, 32x32, 48x48, 64x64, 128x128, and 256x256.
/// Encodes each resolution with 32-bit RGBA PNG, fully compliant with Windows Vista, 7, 8, 10, 11
/// icon specifications for high-DPI and standard display scaling.
pub fn create_authentic_ico(img: &image::DynamicImage, output: &Path) -> Result<(), ConvertError> {
    const SIZES: &[u32] = &[256, 128, 64, 48, 32, 24, 16];

    struct IcoEntry {
        size: u32,
        data: Vec<u8>,
    }

    let mut entries = Vec::with_capacity(SIZES.len());
    for &size in SIZES {
        let resized = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        let mut png_bytes = Vec::new();
        resized
            .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to encode {}x{} PNG for ICO: {}", size, size, e)))?;
        entries.push(IcoEntry {
            size,
            data: png_bytes,
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

    // 3. Image Payloads (PNG data chunks)
    for entry in &entries {
        ico_bytes.extend_from_slice(&entry.data);
    }

    std::fs::write(output, ico_bytes).map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}
