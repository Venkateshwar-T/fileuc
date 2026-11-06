use fileuc_types::{ConvertError, ImageFormat};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct ImageOptions {
    pub quality: Option<u8>,
    pub resize: Option<ResizeSpec>,
    pub grayscale: bool,
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

    let target_ext = match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::WebP => "webp",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Ico => "ico",
        ImageFormat::Tiff => "tiff",
        ImageFormat::Avif => "avif",
    };

    let out_path = crate::output::resolve_output_path(src, target_ext);

    match format {
        ImageFormat::Jpeg => {
            let quality = opts.quality.unwrap_or(90);
            let mut file = std::fs::File::create(&out_path)
                .map_err(|e| ConvertError::IoError(e.to_string()))?;
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, quality);
            encoder
                .encode_image(&img)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
        }
        ImageFormat::Ico => {
            // ICO max dimensions are 256x256
            let ico_img = if img.width() > 256 || img.height() > 256 {
                img.resize(256, 256, image::imageops::FilterType::Lanczos3)
            } else {
                img
            };
            ico_img
                .save(&out_path)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
        }
        _ => {
            img.save(&out_path)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
        }
    }

    Ok(out_path)
}
