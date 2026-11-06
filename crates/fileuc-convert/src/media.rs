use fileuc_types::ConvertError;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn find_ffmpeg() -> Option<PathBuf> {
    if let Ok(output) = Command::new("ffmpeg").arg("-version").output() {
        if output.status.success() {
            return Some(PathBuf::from("ffmpeg"));
        }
    }
    None
}

pub fn video_to_gif(ffmpeg: &Path, src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let status = Command::new(ffmpeg)
        .args([
            "-i",
            src.to_str().unwrap_or_default(),
            "-vf",
            "fps=15,scale=480:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse",
            "-y",
            output.to_str().unwrap_or_default(),
        ])
        .status()
        .map_err(|e| ConvertError::IoError(format!("Failed to spawn ffmpeg: {}", e)))?;

    if !status.success() {
        return Err(ConvertError::ConverterError("FFmpeg video to GIF conversion failed".to_string()));
    }
    Ok(())
}

pub fn extract_audio(ffmpeg: &Path, src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let status = Command::new(ffmpeg)
        .args([
            "-i",
            src.to_str().unwrap_or_default(),
            "-vn",
            "-c:a",
            "libmp3lame",
            "-q:a",
            "2",
            "-y",
            output.to_str().unwrap_or_default(),
        ])
        .status()
        .map_err(|e| ConvertError::IoError(format!("Failed to spawn ffmpeg: {}", e)))?;

    if !status.success() {
        return Err(ConvertError::ConverterError("FFmpeg audio extraction failed".to_string()));
    }
    Ok(())
}

pub fn compress_video(ffmpeg: &Path, src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let status = Command::new(ffmpeg)
        .args([
            "-i",
            src.to_str().unwrap_or_default(),
            "-vcodec",
            "libx264",
            "-crf",
            "28",
            "-preset",
            "fast",
            "-y",
            output.to_str().unwrap_or_default(),
        ])
        .status()
        .map_err(|e| ConvertError::IoError(format!("Failed to spawn ffmpeg: {}", e)))?;

    if !status.success() {
        return Err(ConvertError::ConverterError("FFmpeg video compression failed".to_string()));
    }
    Ok(())
}
