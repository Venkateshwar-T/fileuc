use crate::media::find_ffmpeg;
use crate::pdfium::init_pdfium;
use fileuc_types::ConvertError;
use image::ImageEncoder;
use lopdf::{Document, Object};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Generates a reduced output path with `_reduced` appended before the extension:
/// e.g. `C:\Videos\demo.mp4` -> `C:\Videos\demo_reduced.mp4`
pub fn reduced_output_path(src: &Path) -> PathBuf {
    let parent = src.parent().unwrap_or_else(|| Path::new(""));
    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    let new_name = format!("{}_reduced{}", stem, ext);
    parent.join(new_name)
}

/// Reduces a photo, PDF, Office document, or video file to stay under `target_bytes`.
/// Guaranteed invariant: The generated file will ALWAYS be strictly smaller than the original.
pub fn reduce_size(
    src: &Path,
    output: Option<&Path>,
    target_bytes: u64,
) -> Result<PathBuf, ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    if target_bytes == 0 {
        return Err(ConvertError::ConverterError("Target size must be greater than zero".to_string()));
    }

    let orig_size = src.metadata().map(|m| m.len()).unwrap_or(0);
    if orig_size > 0 && target_bytes >= orig_size {
        return Err(ConvertError::ConverterError(format!(
            "Target size ({} bytes) must be smaller than current file size ({} bytes)",
            target_bytes, orig_size
        )));
    }

    let out_path = output
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| reduced_output_path(src));
    let guard = crate::output_guard::OutputGuard::new(&out_path);

    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        // Photos / Images
        "jpg" | "jpeg" | "png" | "webp" | "bmp" | "tiff" | "tif" => {
            reduce_image_size(src, &out_path, target_bytes, orig_size)?;
        }
        // Videos
        "mp4" | "mkv" | "mov" | "avi" | "webm" => {
            reduce_video_size(src, &out_path, target_bytes, orig_size)?;
        }
        // PDFs
        "pdf" => {
            reduce_pdf_size(src, &out_path, target_bytes, orig_size)?;
        }
        // Office Documents
        "docx" | "xlsx" | "pptx" => {
            reduce_office_size(src, &out_path, target_bytes, orig_size)?;
        }
        _ => {
            return Err(ConvertError::UnsupportedFormat(format!(
                "Cannot reduce size for extension .{}",
                ext
            )));
        }
    }

    // Strict safety verification: The output file MUST be strictly smaller than the original!
    let final_size = out_path.metadata().map(|m| m.len()).unwrap_or(u64::MAX);
    if orig_size > 0 && final_size >= orig_size {
        let _ = std::fs::remove_file(&out_path);
        return Err(ConvertError::ConverterError(format!(
            "File is already optimally compressed and cannot be reduced further (original: {} bytes, reduced: {} bytes)",
            orig_size, final_size
        )));
    }

    guard.disarm();
    Ok(out_path)
}

fn reduce_image_size(
    src: &Path,
    dest: &Path,
    target_bytes: u64,
    orig_size: u64,
) -> Result<(), ConvertError> {
    let img = crate::image::open_image_with_orientation(src)?;

    let ext = dest
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_else(|| "jpg".to_string());

    let scale_factors = [1.0f32, 0.85, 0.70, 0.55, 0.40, 0.25];
    let qualities = [82u8, 70, 55, 40, 25, 15];

    let mut best_buffer: Option<Vec<u8>> = None;

    let orig_w = img.width();
    let orig_h = img.height();

    for &scale in &scale_factors {
        let current_img = if (scale - 1.0).abs() < 0.01 {
            img.clone()
        } else {
            let nw = (orig_w as f32 * scale).max(32.0) as u32;
            let nh = (orig_h as f32 * scale).max(32.0) as u32;
            img.resize(nw, nh, image::imageops::FilterType::Lanczos3)
        };

        if ext == "png" {
            // PNG is lossless; scale down dimensions and try encoding
            let mut buf = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut buf);
            if current_img.write_to(&mut cursor, image::ImageFormat::Png).is_ok() {
                let len = buf.len() as u64;
                if len <= target_bytes && (orig_size == 0 || len < orig_size) {
                    std::fs::write(dest, buf).map_err(|e| ConvertError::IoError(e.to_string()))?;
                    return Ok(());
                }
                if orig_size == 0 || len < orig_size {
                    if best_buffer.as_ref().map(|b| b.len() > buf.len()).unwrap_or(true) {
                        best_buffer = Some(buf);
                    }
                }
            }
        } else if ext == "webp" {
            let mut buf = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut buf);
            if current_img.write_to(&mut cursor, image::ImageFormat::WebP).is_ok() {
                let len = buf.len() as u64;
                if len <= target_bytes && (orig_size == 0 || len < orig_size) {
                    std::fs::write(dest, buf).map_err(|e| ConvertError::IoError(e.to_string()))?;
                    return Ok(());
                }
                if orig_size == 0 || len < orig_size {
                    if best_buffer.as_ref().map(|b| b.len() > buf.len()).unwrap_or(true) {
                        best_buffer = Some(buf);
                    }
                }
            }
        } else {
            // JPEG / others: sweep through qualities
            for &quality in &qualities {
                let mut buf = Vec::new();
                let mut cursor = std::io::Cursor::new(&mut buf);
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
                let encode_res = encoder.write_image(
                    current_img.as_bytes(),
                    current_img.width(),
                    current_img.height(),
                    current_img.color().into(),
                );

                if encode_res.is_ok() && !buf.is_empty() {
                    let len = buf.len() as u64;
                    if len <= target_bytes && (orig_size == 0 || len < orig_size) {
                        std::fs::write(dest, buf).map_err(|e| ConvertError::IoError(e.to_string()))?;
                        return Ok(());
                    }
                    if orig_size == 0 || len < orig_size {
                        if best_buffer.as_ref().map(|b| b.len() > buf.len()).unwrap_or(true) {
                            best_buffer = Some(buf);
                        }
                    }
                }
            }
        }
    }

    if let Some(buf) = best_buffer {
        if orig_size == 0 || (buf.len() as u64) < orig_size {
            std::fs::write(dest, buf).map_err(|e| ConvertError::IoError(e.to_string()))?;
            return Ok(());
        }
    }

    Err(ConvertError::ConverterError(
        "Image is already compressed to maximum efficiency and cannot be reduced further".to_string(),
    ))
}

fn get_video_duration(ffmpeg: &Path, src: &Path) -> Option<f64> {
    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-i").arg(src);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd.output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    if let Some(idx) = stderr.find("Duration: ") {
        let after = &stderr[idx + 10..];
        if let Some(end) = after.find(',') {
            let dur_str = after[..end].trim();
            let parts: Vec<&str> = dur_str.split(':').collect();
            if parts.len() == 3 {
                let h: f64 = parts[0].parse().unwrap_or(0.0);
                let m: f64 = parts[1].parse().unwrap_or(0.0);
                let s: f64 = parts[2].parse().unwrap_or(0.0);
                let total = h * 3600.0 + m * 60.0 + s;
                if total > 0.05 {
                    return Some(total);
                }
            }
        }
    }
    None
}

fn reduce_video_size(
    src: &Path,
    dest: &Path,
    target_bytes: u64,
    orig_size: u64,
) -> Result<(), ConvertError> {
    let ffmpeg = find_ffmpeg().ok_or_else(|| {
        ConvertError::EngineMissing("FFmpeg is required to compress video".to_string())
    })?;

    let duration = get_video_duration(&ffmpeg, src).unwrap_or(30.0);

    let target_bits = (target_bytes as f64 * 8.0 * 0.92).max(10_000.0);
    let total_kbps = (target_bits / duration / 1000.0) as i64;

    let audio_kbps = if total_kbps < 200 { 48 } else if total_kbps < 500 { 64 } else { 96 };
    let video_kbps = (total_kbps - audio_kbps).max(64);

    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-i").arg(src);

    if video_kbps < 350 {
        cmd.arg("-vf").arg("scale=-2:480");
    } else if video_kbps < 750 {
        cmd.arg("-vf").arg("scale=-2:720");
    }

    cmd.arg("-c:v")
        .arg("libx264")
        .arg("-b:v")
        .arg(format!("{}k", video_kbps))
        .arg("-maxrate")
        .arg(format!("{}k", (video_kbps as f64 * 1.2) as i64))
        .arg("-bufsize")
        .arg(format!("{}k", video_kbps * 2))
        .arg("-preset")
        .arg("fast")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg(format!("{}k", audio_kbps))
        .arg("-y")
        .arg(dest);

    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let status = cmd
        .status()
        .map_err(|e| ConvertError::IoError(format!("Failed to spawn FFmpeg: {}", e)))?;

    if !status.success() {
        return Err(ConvertError::ConverterError("FFmpeg video reduction failed".to_string()));
    }

    let dest_size = dest.metadata().map(|m| m.len()).unwrap_or(u64::MAX);
    if orig_size > 0 && dest_size >= orig_size {
        let _ = std::fs::remove_file(dest);
        return Err(ConvertError::ConverterError(
            "Video is already encoded at optimal bitrate and cannot be reduced further".to_string(),
        ));
    }

    Ok(())
}

fn reduce_pdf_size(
    src: &Path,
    dest: &Path,
    target_bytes: u64,
    orig_size: u64,
) -> Result<(), ConvertError> {
    // Attempt 1: In-Place lopdf Stream Optimization (recompresses all embedded images without losing vector text)
    let temp_stage1 = dest.with_extension("stage1.tmp");
    let stage1_res = optimize_pdf_stream_images(src, &temp_stage1, target_bytes, orig_size);

    let stage1_size = if stage1_res.is_ok() && temp_stage1.exists() {
        temp_stage1.metadata().map(|m| m.len()).unwrap_or(u64::MAX)
    } else {
        u64::MAX
    };

    // If Stage 1 succeeded, reduced the file, AND hit <= target_bytes, we're done!
    if stage1_size <= target_bytes && (orig_size == 0 || stage1_size < orig_size) {
        let _ = std::fs::rename(&temp_stage1, dest);
        return Ok(());
    }

    let _ = std::fs::remove_file(&temp_stage1);

    // Attempt 2: High-Quality Page Rasterization via Pdfium & PDF-Writer
    // Guarantees meeting <= target_bytes by computing an exact per-page budget
    let temp_stage2 = dest.with_extension("stage2.tmp");
    let stage2_res = raster_reduce_pdf(src, &temp_stage2, target_bytes, orig_size);

    let stage2_size = if stage2_res.is_ok() && temp_stage2.exists() {
        temp_stage2.metadata().map(|m| m.len()).unwrap_or(u64::MAX)
    } else {
        u64::MAX
    };

    if stage2_size <= target_bytes && (orig_size == 0 || stage2_size < orig_size) {
        let _ = std::fs::rename(&temp_stage2, dest);
        return Ok(());
    }

    let _ = std::fs::remove_file(&temp_stage2);
    Err(ConvertError::ConverterError(format!(
        "Could not reduce file to under {} KB without severe content loss",
        target_bytes / 1024
    )))
}

fn optimize_pdf_stream_images(
    src: &Path,
    dest: &Path,
    target_bytes: u64,
    orig_size: u64,
) -> Result<(), ConvertError> {
    let mut doc = Document::load(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to load PDF: {}", e)))?;

    let ratio = if orig_size > 0 {
        (target_bytes as f64 / orig_size as f64).clamp(0.1, 0.9)
    } else {
        0.5
    };

    let target_jpeg_quality = (70.0 * ratio).clamp(25.0, 75.0) as u8;
    let max_dim = (1400.0 * ratio.sqrt()).max(600.0) as u32;

    for (_, obj) in doc.objects.iter_mut() {
        if let Object::Stream(ref mut stream) = obj {
            let is_image = stream
                .dict
                .get(b"Subtype")
                .and_then(|v| v.as_name())
                .map(|name| name == b"Image")
                .unwrap_or(false);

            if !is_image {
                continue;
            }

            let width = stream
                .dict
                .get(b"Width")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as u32;
            let height = stream
                .dict
                .get(b"Height")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as u32;

            if width < 32 || height < 32 {
                continue;
            }

            let filter_name: &[u8] = stream
                .dict
                .get(b"Filter")
                .and_then(|v| v.as_name())
                .unwrap_or(b"".as_slice());

            let colorspace_name: &[u8] = stream
                .dict
                .get(b"ColorSpace")
                .and_then(|v| v.as_name())
                .unwrap_or(b"".as_slice());

            // Case A: FlateDecode (uncompressed raster bitmap)
            if filter_name == b"FlateDecode" || filter_name.is_empty() {
                if let Ok(raw_pixels) = stream.decompressed_content() {
                    let mut jpeg_bytes = Vec::new();
                    let success = if (colorspace_name == b"DeviceRGB" || colorspace_name.is_empty())
                        && raw_pixels.len() == (width * height * 3) as usize
                    {
                        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(
                            &mut jpeg_bytes,
                            target_jpeg_quality,
                        );
                        enc.encode(&raw_pixels, width, height, image::ExtendedColorType::Rgb8)
                            .is_ok()
                    } else if colorspace_name == b"DeviceGray"
                        && raw_pixels.len() == (width * height) as usize
                    {
                        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(
                            &mut jpeg_bytes,
                            target_jpeg_quality,
                        );
                        enc.encode(&raw_pixels, width, height, image::ExtendedColorType::L8)
                            .is_ok()
                    } else {
                        false
                    };

                    if success && !jpeg_bytes.is_empty() && jpeg_bytes.len() < stream.content.len() {
                        stream.set_content(jpeg_bytes);
                        stream.dict.set(b"Filter", Object::Name(b"DCTDecode".to_vec()));
                        stream.dict.remove(b"DecodeParms");
                        continue;
                    }
                }
            }

            // Case B: DCTDecode (recompress existing JPEG)
            if filter_name == b"DCTDecode" {
                if let Ok(img) = image::load_from_memory_with_format(
                    &stream.content,
                    image::ImageFormat::Jpeg,
                ) {
                    let (cur_w, cur_h) = (img.width(), img.height());
                    let (resized, _) = if cur_w > max_dim || cur_h > max_dim {
                        let scale = (max_dim as f32 / cur_w.max(cur_h) as f32).min(1.0);
                        let nw = (cur_w as f32 * scale).max(64.0) as u32;
                        let nh = (cur_h as f32 * scale).max(64.0) as u32;
                        (
                            img.resize(nw, nh, image::imageops::FilterType::Lanczos3),
                            true,
                        )
                    } else {
                        (img, false)
                    };

                    let act_w = resized.width();
                    let act_h = resized.height();
                    let mut new_jpeg = Vec::new();
                    let rgb = resized.to_rgb8();
                    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(
                        &mut new_jpeg,
                        target_jpeg_quality,
                    );
                    if enc
                        .encode(&rgb, act_w, act_h, image::ExtendedColorType::Rgb8)
                        .is_ok()
                    {
                        if !new_jpeg.is_empty() && new_jpeg.len() < stream.content.len() {
                            stream.set_content(new_jpeg);
                            stream.dict.set(b"Width", Object::Integer(act_w as i64));
                            stream.dict.set(b"Height", Object::Integer(act_h as i64));
                        }
                    }
                }
            }
        }
    }

    doc.prune_objects();
    doc.compress();

    doc.save(dest)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to save optimized PDF: {}", e)))?;
    Ok(())
}

fn raster_reduce_pdf(
    src: &Path,
    dest: &Path,
    target_bytes: u64,
    _orig_size: u64,
) -> Result<(), ConvertError> {
    let pdfium = init_pdfium()?;
    let doc = pdfium
        .load_pdf_from_file(src, None)
        .or_else(|_| pdfium.load_pdf_from_file(src, Some("")))
        .map_err(|e| ConvertError::ConverterError(format!("Pdfium failed to open PDF: {}", e)))?;

    let pages_len = doc.pages().len() as usize;
    if pages_len == 0 {
        return Err(ConvertError::ConverterError("PDF contains 0 pages".to_string()));
    }

    // Overhead for PDF dictionary tree with N pages in pdf_writer
    let overhead = 1500u64 + (pages_len as u64) * 900;
    let total_jpeg_budget = target_bytes.saturating_sub(overhead);

    struct PageRecord {
        page_ref: Ref,
        image_ref: Ref,
        width: f32,
        height: f32,
        pixel_w: u32,
        pixel_h: u32,
        jpeg_data: Vec<u8>,
        is_gray: bool,
    }

    let presets: [(i32, u8); 10] = [
        (1400, 75),
        (1200, 65),
        (1024, 55),
        (850, 45),
        (720, 35),
        (580, 25),
        (460, 18),
        (380, 14),
        (320, 11),
        (260, 8),
    ];

    for &(target_width, quality) in &presets {
        let mut pdf = Pdf::new();
        let mut page_refs = Vec::new();
        let mut next_ref = 1;

        let catalog_ref = Ref::new(next_ref);
        next_ref += 1;
        let page_tree_ref = Ref::new(next_ref);
        next_ref += 1;

        let mut records = Vec::new();
        let mut total_jpeg_bytes = 0u64;
        let mut render_failed = false;

        for i in 0..pages_len {
            let img = match crate::pdfium::render_page_to_image_from_doc(&doc, i, target_width) {
                Ok(im) => im,
                Err(_) => {
                    render_failed = true;
                    break;
                }
            };
            let pw = img.width();
            let ph = img.height();

            let rgb_img = img.to_rgb8();

            // Detect if page is grayscale (black/gray text) to save ~65% space
            let is_color = rgb_img.pixels().step_by(16).any(|p| {
                let diff = (p[0] as i32 - p[1] as i32).abs() + (p[1] as i32 - p[2] as i32).abs();
                diff > 18
            });

            let (jpeg_bytes, is_gray) = if !is_color {
                let gray_img = image::DynamicImage::ImageRgb8(rgb_img).into_luma8();
                let mut jbytes = Vec::new();
                let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jbytes, quality);
                if enc.encode(&gray_img, pw, ph, image::ExtendedColorType::L8).is_err() {
                    render_failed = true;
                    break;
                }
                (jbytes, true)
            } else {
                let mut jbytes = Vec::new();
                let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jbytes, quality);
                if enc.encode(&rgb_img, pw, ph, image::ExtendedColorType::Rgb8).is_err() {
                    render_failed = true;
                    break;
                }
                (jbytes, false)
            };

            total_jpeg_bytes += jpeg_bytes.len() as u64;

            let page_ref = Ref::new(next_ref);
            next_ref += 1;
            let image_ref = Ref::new(next_ref);
            next_ref += 1;

            page_refs.push(page_ref);
            records.push(PageRecord {
                page_ref,
                image_ref,
                width: pw as f32,
                height: ph as f32,
                pixel_w: pw,
                pixel_h: ph,
                jpeg_data: jpeg_bytes,
                is_gray,
            });
        }

        if render_failed {
            continue;
        }

        // If this preset's JPEG bytes exceed the total budget, try the next lower resolution/quality preset!
        if total_jpeg_bytes > total_jpeg_budget && presets.last() != Some(&(target_width, quality)) {
            continue;
        }

        pdf.catalog(catalog_ref).pages(page_tree_ref);
        pdf.pages(page_tree_ref)
            .kids(page_refs.iter().copied())
            .count(page_refs.len() as i32);

        for r in records {
            let content_ref = Ref::new(next_ref);
            next_ref += 1;
            let image_name = Name(b"Im0");

            let mut page = pdf.page(r.page_ref);
            page.media_box(Rect::new(0.0, 0.0, r.width, r.height));
            page.parent(page_tree_ref);
            page.contents(content_ref);
            page.resources().x_objects().pair(image_name, r.image_ref);
            page.finish();

            let mut content = Content::new();
            content.save_state();
            content.transform([r.width, 0.0, 0.0, r.height, 0.0, 0.0]);
            content.x_object(image_name);
            content.restore_state();
            pdf.stream(content_ref, &content.finish());

            let mut image_stream = pdf.image_xobject(r.image_ref, &r.jpeg_data);
            image_stream.width(r.pixel_w as i32);
            image_stream.height(r.pixel_h as i32);
            if r.is_gray {
                image_stream.color_space().device_gray();
            } else {
                image_stream.color_space().device_rgb();
            }
            image_stream.bits_per_component(8);
            image_stream.filter(Filter::DctDecode);
            image_stream.finish();
        }

        let pdf_data = pdf.finish();
        if (pdf_data.len() as u64) <= target_bytes {
            std::fs::write(dest, pdf_data).map_err(|e| ConvertError::IoError(e.to_string()))?;
            return Ok(());
        }
    }

    Err(ConvertError::ConverterError("Failed to rasterize and compress PDF to target size".to_string()))
}

fn reduce_office_size(
    src: &Path,
    dest: &Path,
    target_bytes: u64,
    orig_size: u64,
) -> Result<(), ConvertError> {
    let src_file = std::fs::File::open(src)
        .map_err(|e| ConvertError::IoError(format!("Failed to open document: {}", e)))?;
    let mut archive = ZipArchive::new(src_file)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to read archive: {}", e)))?;

    let dest_file = std::fs::File::create(dest)
        .map_err(|e| ConvertError::IoError(format!("Failed to create destination: {}", e)))?;
    let mut zip_writer = ZipWriter::new(dest_file);

    let ratio = if orig_size > 0 {
        (target_bytes as f64 / orig_size as f64).clamp(0.1, 0.9)
    } else {
        0.5
    };
    let target_jpeg_quality = (70.0 * ratio).clamp(25.0, 75.0) as u8;
    let max_dim = (1280.0 * ratio.sqrt()).max(600.0) as u32;

    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| ConvertError::ConverterError(format!("Zip read error: {}", e)))?;
        let name = file.name().to_string();

        let mut content = Vec::new();
        file.read_to_end(&mut content)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;

        // Optimize embedded media images in word/media/, ppt/media/, xl/media/
        let is_media = (name.starts_with("word/media/")
            || name.starts_with("ppt/media/")
            || name.starts_with("xl/media/"))
            && (name.ends_with(".png")
                || name.ends_with(".jpg")
                || name.ends_with(".jpeg")
                || name.ends_with(".bmp"));

        if is_media {
            if let Ok(img) = image::load_from_memory(&content) {
                let (w, h) = (img.width(), img.height());
                let (resized, nw, nh) = if w > max_dim || h > max_dim {
                    let s = (max_dim as f32 / w.max(h) as f32).min(1.0);
                    let nw = (w as f32 * s).max(64.0) as u32;
                    let nh = (h as f32 * s).max(64.0) as u32;
                    (img.resize(nw, nh, image::imageops::FilterType::Lanczos3), nw, nh)
                } else {
                    (img, w, h)
                };

                let mut new_jpeg = Vec::new();
                let rgb = resized.to_rgb8();
                let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(
                    &mut new_jpeg,
                    target_jpeg_quality,
                );
                if enc.encode(&rgb, nw, nh, image::ExtendedColorType::Rgb8).is_ok() {
                    if !new_jpeg.is_empty() && new_jpeg.len() < content.len() {
                        content = new_jpeg;
                    }
                }
            }
        }

        zip_writer
            .start_file(&name, options)
            .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
        zip_writer
            .write_all(&content)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
    }

    zip_writer
        .finish()
        .map_err(|e| ConvertError::ConverterError(format!("Failed to finalize document: {}", e)))?;

    let dest_size = dest.metadata().map(|m| m.len()).unwrap_or(u64::MAX);
    if orig_size > 0 && dest_size >= orig_size {
        let _ = std::fs::remove_file(dest);
        return Err(ConvertError::ConverterError(
            "Document is already compressed to maximum efficiency and cannot be reduced further".to_string(),
        ));
    }

    Ok(())
}
