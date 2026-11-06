use fileuc_types::{ConvertError, ImageFormat};
use std::path::{Path, PathBuf};

pub fn render_svg(
    src: &Path,
    format: ImageFormat,
    target_width: Option<u32>,
) -> Result<PathBuf, ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let svg_data = std::fs::read(src).map_err(|e| ConvertError::IoError(e.to_string()))?;

    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(&svg_data, &opt)
        .map_err(|e| ConvertError::ConverterError(format!("Invalid SVG: {}", e)))?;

    let size = tree.size();
    let mut width = size.width() as u32;
    let mut height = size.height() as u32;

    if width == 0 || height == 0 {
        width = 512;
        height = 512;
    }

    let (final_w, final_h, transform) = if let Some(tw) = target_width {
        let scale = tw as f32 / width as f32;
        let th = ((height as f32) * scale).round() as u32;
        (
            tw.max(1),
            th.max(1),
            tiny_skia::Transform::from_scale(scale, scale),
        )
    } else {
        (width, height, tiny_skia::Transform::identity())
    };

    let mut pixmap = tiny_skia::Pixmap::new(final_w, final_h)
        .ok_or_else(|| ConvertError::ConverterError("Failed to allocate pixmap".to_string()))?;

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let ext = match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::WebP => "webp",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Ico => "ico",
        ImageFormat::Tiff => "tiff",
        ImageFormat::Avif => "avif",
    };

    let out_path = crate::output::resolve_output_path(src, ext);

    // Convert Pixmap RGBA premultiplied to standard RgbaImage
    let mut rgba_buf = pixmap.take();
    // Un-premultiply alpha for image crate
    for pixel in rgba_buf.chunks_exact_mut(4) {
        let a = pixel[3] as f32 / 255.0;
        if a > 0.0 {
            pixel[0] = ((pixel[0] as f32 / a).min(255.0)) as u8;
            pixel[1] = ((pixel[1] as f32 / a).min(255.0)) as u8;
            pixel[2] = ((pixel[2] as f32 / a).min(255.0)) as u8;
        }
    }

    let dyn_img = image::RgbaImage::from_raw(final_w, final_h, rgba_buf)
        .map(image::DynamicImage::ImageRgba8)
        .ok_or_else(|| ConvertError::ConverterError("Failed to create image buffer".to_string()))?;

    match format {
        ImageFormat::Jpeg => {
            let mut file = std::fs::File::create(&out_path)
                .map_err(|e| ConvertError::IoError(e.to_string()))?;
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 90);
            encoder
                .encode_image(&dyn_img)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
        }
        _ => {
            dyn_img
                .save(&out_path)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
        }
    }

    Ok(out_path)
}
