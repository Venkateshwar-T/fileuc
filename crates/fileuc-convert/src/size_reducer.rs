use crate::media::find_ffmpeg;
use fileuc_types::ConvertError;
use image::ImageEncoder;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Reduces a photo, PDF, or video file to stay under `target_bytes`.
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
            reduce_image_size(src, &out_path, target_bytes)?;
        }
        // Videos
        "mp4" | "mkv" | "mov" | "avi" | "webm" => {
            reduce_video_size(src, &out_path, target_bytes)?;
        }
        // PDFs
        "pdf" => {
            reduce_pdf_size(src, &out_path, target_bytes)?;
        }
        _ => {
            return Err(ConvertError::UnsupportedFormat(format!(
                "Cannot reduce size for extension .{}",
                ext
            )));
        }
    }

    guard.disarm();
    Ok(out_path)
}

fn reduce_image_size(src: &Path, dest: &Path, target_bytes: u64) -> Result<(), ConvertError> {
    let mut img = image::open(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to open image: {}", e)))?;

    let ext = dest
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_else(|| "jpg".to_string());

    // Iteratively adjust JPEG/WebP quality and scale dimensions if needed
    let qualities = [85u8, 70, 55, 40, 25, 15];
    let mut best_buffer: Option<Vec<u8>> = None;

    for scale_pass in 0..4 {
        if scale_pass > 0 {
            let (w, h) = (img.width(), img.height());
            let new_w = (w as f32 * 0.75).max(64.0) as u32;
            let new_h = (h as f32 * 0.75).max(64.0) as u32;
            img = img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3);
        }

        for &quality in &qualities {
            let mut buf = Vec::new();
            let encode_res = if ext == "png" {
                // For PNG, try saving with compression or convert to indexed/RGB
                let mut cursor = std::io::Cursor::new(&mut buf);
                img.write_to(&mut cursor, image::ImageFormat::Png)
            } else if ext == "webp" {
                let mut cursor = std::io::Cursor::new(&mut buf);
                img.write_to(&mut cursor, image::ImageFormat::WebP)
            } else {
                let mut cursor = std::io::Cursor::new(&mut buf);
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
                encoder.write_image(
                    img.as_bytes(),
                    img.width(),
                    img.height(),
                    img.color().into(),
                )
            };

            if encode_res.is_ok() && !buf.is_empty() {
                let len = buf.len() as u64;
                if len <= target_bytes {
                    std::fs::write(dest, buf)
                        .map_err(|e| ConvertError::IoError(e.to_string()))?;
                    return Ok(());
                }
                if best_buffer.as_ref().map(|b| b.len() > buf.len()).unwrap_or(true) {
                    best_buffer = Some(buf);
                }
            }
        }
    }

    // If still slightly over, write the best compressed version we achieved
    if let Some(buf) = best_buffer {
        std::fs::write(dest, buf)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
        Ok(())
    } else {
        Err(ConvertError::ConverterError("Unable to compress image to specified target size".to_string()))
    }
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

    // Look for "Duration: 00:01:23.45"
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

fn reduce_video_size(src: &Path, dest: &Path, target_bytes: u64) -> Result<(), ConvertError> {
    let ffmpeg = find_ffmpeg().ok_or_else(|| {
        ConvertError::EngineMissing("FFmpeg is required to compress video".to_string())
    })?;

    let duration = get_video_duration(&ffmpeg, src).unwrap_or(30.0);

    // Leave 5% margin for container overhead
    let target_bits = (target_bytes as f64 * 8.0 * 0.95).max(10_000.0);
    let total_kbps = (target_bits / duration / 1000.0) as i64;

    let audio_kbps = if total_kbps < 200 { 48 } else if total_kbps < 500 { 64 } else { 96 };
    let video_kbps = (total_kbps - audio_kbps).max(80);

    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-i").arg(src);

    // If bitrate is low, downscale to 480p or 360p to preserve visual clarity
    if video_kbps < 400 {
        cmd.arg("-vf").arg("scale=-2:480");
    } else if video_kbps < 800 {
        cmd.arg("-vf").arg("scale=-2:720");
    }

    cmd.arg("-c:v")
        .arg("libx264")
        .arg("-b:v")
        .arg(format!("{}k", video_kbps))
        .arg("-maxrate")
        .arg(format!("{}k", (video_kbps as f64 * 1.25) as i64))
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

    Ok(())
}

fn reduce_pdf_size(src: &Path, dest: &Path, _target_bytes: u64) -> Result<(), ConvertError> {
    // 1. Check if Ghostscript is installed
    let candidates = [
        PathBuf::from(r"C:\Program Files\gs\gs10.04.0\bin\gswin64c.exe"),
        PathBuf::from(r"C:\Program Files\gs\gs10.03.0\bin\gswin64c.exe"),
        PathBuf::from(r"C:\Program Files\gs\gs10.02.0\bin\gswin64c.exe"),
        PathBuf::from("gswin64c.exe"),
    ];

    for gs in &candidates {
        if gs.exists() {
            let mut cmd = Command::new(gs);
            cmd.arg("-sDEVICE=pdfwrite")
                .arg("-dCompatibilityLevel=1.4")
                .arg("-dPDFSETTINGS=/ebook")
                .arg("-dNOPAUSE")
                .arg("-dQUIET")
                .arg("-dBATCH")
                .arg(format!("-sOutputFile={}", dest.display()))
                .arg(src);
            #[cfg(windows)]
            {
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            if let Ok(status) = cmd.status() {
                if status.success() && dest.exists() {
                    return Ok(());
                }
            }
        }
    }

    // 2. Fallback: use internal pdf compress
    crate::pdf::compress_pdf(src, dest)
}
