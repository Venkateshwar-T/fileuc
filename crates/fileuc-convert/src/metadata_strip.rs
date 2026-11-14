use crate::media::find_ffmpeg;
use fileuc_types::ConvertError;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Generates a clean output path with `_clean` appended before the extension:
/// e.g. `C:\Photos\DSC_001.jpg` -> `C:\Photos\DSC_001_clean.jpg`
pub fn clean_output_path(src: &Path) -> PathBuf {
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

    let new_name = format!("{}_clean{}", stem, ext);
    parent.join(new_name)
}

/// Strips all hidden metadata (GPS coordinates, camera model, date taken, artist,
/// album, cover art, chapters, etc.) from photo, audio, or video files, creating
/// a clean new file with `_clean` appended in the file name.
pub fn strip_metadata(src: &Path, output: Option<&Path>) -> Result<PathBuf, ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let out_path = output
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| clean_output_path(src));

    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        // Photos / Images
        "jpg" | "jpeg" | "png" | "webp" | "bmp" | "tiff" | "tif" => {
            strip_image_metadata(src, &out_path)?;
        }
        // Audio
        "mp3" | "wav" | "ogg" | "aac" | "flac" | "m4a" | "wma" | "opus" | "aiff" => {
            strip_audio_metadata(src, &out_path)?;
        }
        // Video
        "mp4" | "mkv" | "mov" | "avi" | "webm" => {
            strip_video_metadata(src, &out_path)?;
        }
        // Fallback: if unknown or other formats, try FFmpeg stream copy metadata strip
        _ => {
            if let Some(ffmpeg) = find_ffmpeg() {
                let mut cmd = Command::new(&ffmpeg);
                cmd.arg("-i")
                    .arg(src)
                    .arg("-map_metadata")
                    .arg("-1")
                    .arg("-c")
                    .arg("copy")
                    .arg("-y")
                    .arg(&out_path);
                #[cfg(windows)]
                {
                    cmd.creation_flags(CREATE_NO_WINDOW);
                }
                let status = cmd.status().map_err(|e| ConvertError::IoError(e.to_string()))?;
                if !status.success() {
                    return Err(ConvertError::ConverterError("Failed to strip metadata with FFmpeg".to_string()));
                }
            } else {
                return Err(ConvertError::UnsupportedFormat(ext));
            }
        }
    }

    Ok(out_path)
}

fn strip_image_metadata(src: &Path, dest: &Path) -> Result<(), ConvertError> {
    // 1. If FFmpeg is available and the image is JPEG, try lossless stream copy first
    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    if ext == "jpg" || ext == "jpeg" {
        if let Some(ffmpeg) = find_ffmpeg() {
            let mut cmd = Command::new(&ffmpeg);
            cmd.arg("-i")
                .arg(src)
                .arg("-map_metadata")
                .arg("-1")
                .arg("-c:v")
                .arg("copy")
                .arg("-y")
                .arg(dest);
            #[cfg(windows)]
            {
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            if let Ok(status) = cmd.status() {
                if status.success() && dest.exists() && std::fs::metadata(dest).map(|m| m.len() > 0).unwrap_or(false) {
                    return Ok(());
                }
            }
        }
    }

    // 2. Decode and re-encode pixel buffer: standard image crate encoders write zero metadata chunks
    let img = image::open(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to open image for stripping: {}", e)))?;

    img.save(dest)
        .map_err(|e| ConvertError::IoError(format!("Failed to save clean image: {}", e)))?;

    Ok(())
}

fn strip_audio_metadata(src: &Path, dest: &Path) -> Result<(), ConvertError> {
    let ffmpeg = find_ffmpeg().ok_or_else(|| {
        ConvertError::EngineMissing("FFmpeg is required to strip audio metadata".to_string())
    })?;

    // -vn: drop cover art / embedded album images
    // -map 0:a: map only audio streams
    // -c:a copy: lossless copy
    // -map_metadata -1 -map_metadata:s -1: strip all tags
    // -fflags +bitexact: strip encoder identity / dates
    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-i")
        .arg(src)
        .arg("-vn")
        .arg("-map")
        .arg("0:a?")
        .arg("-c:a")
        .arg("copy")
        .arg("-map_metadata")
        .arg("-1")
        .arg("-map_metadata:s")
        .arg("-1")
        .arg("-fflags")
        .arg("+bitexact")
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
        return Err(ConvertError::ConverterError("FFmpeg audio metadata strip failed".to_string()));
    }

    Ok(())
}

fn strip_video_metadata(src: &Path, dest: &Path) -> Result<(), ConvertError> {
    let ffmpeg = find_ffmpeg().ok_or_else(|| {
        ConvertError::EngineMissing("FFmpeg is required to strip video metadata".to_string())
    })?;

    // -map 0:v? -map 0:a? -map 0:s?: copy video, audio, subtitles (omits data streams like GPS logs)
    // -c copy: lossless stream copy
    // -map_metadata -1 -map_metadata:s -1: strip all tags
    // -map_chapters -1: strip chapters
    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-i")
        .arg(src)
        .arg("-map")
        .arg("0:v?")
        .arg("-map")
        .arg("0:a?")
        .arg("-map")
        .arg("0:s?")
        .arg("-c")
        .arg("copy")
        .arg("-map_metadata")
        .arg("-1")
        .arg("-map_metadata:s")
        .arg("-1")
        .arg("-map_chapters")
        .arg("-1")
        .arg("-fflags")
        .arg("+bitexact")
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
        return Err(ConvertError::ConverterError("FFmpeg video metadata strip failed".to_string()));
    }

    Ok(())
}
