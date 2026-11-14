use fileuc_types::ConvertError;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn find_ffmpeg() -> Option<PathBuf> {
    static CACHED: OnceLock<Option<PathBuf>> = OnceLock::new();
    CACHED
        .get_or_init(|| {
            // 1. Check next to current executable
            if let Ok(exe) = std::env::current_exe() {
                if let Some(parent) = exe.parent() {
                    let next_to_exe = parent.join("ffmpeg.exe");
                    if next_to_exe.exists() {
                        return Some(next_to_exe);
                    }
                    let engines_ffmpeg = parent.join("engines").join("ffmpeg").join("ffmpeg.exe");
                    if engines_ffmpeg.exists() {
                        return Some(engines_ffmpeg);
                    }
                }
            }

            // 2. Check standard system installations
            let candidates = [
                PathBuf::from(r"C:\Program Files\FileUC\engines\ffmpeg\ffmpeg.exe"),
                PathBuf::from(r"C:\Program Files\ffmpeg\bin\ffmpeg.exe"),
                PathBuf::from(r"C:\ffmpeg\bin\ffmpeg.exe"),
                PathBuf::from("ffmpeg.exe"),
            ];
            for c in &candidates {
                if c.exists() {
                    return Some(c.clone());
                }
            }

            // 3. Fall back to system PATH
            let mut cmd = Command::new("ffmpeg");
            cmd.arg("-version");
            #[cfg(windows)]
            {
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            if let Ok(output) = cmd.output() {
                if output.status.success() {
                    return Some(PathBuf::from("ffmpeg"));
                }
            }
            None
        })
        .clone()
}

pub fn video_to_gif(ffmpeg: &Path, src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    let guard = crate::output_guard::OutputGuard::new(output);

    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-i")
        .arg(src)
        .arg("-vf")
        .arg("fps=15,scale=480:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse")
        .arg("-y")
        .arg(output);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let status = cmd
        .status()
        .map_err(|e| ConvertError::IoError(format!("Failed to spawn ffmpeg: {}", e)))?;

    if !status.success() {
        return Err(ConvertError::ConverterError("FFmpeg video to GIF conversion failed".to_string()));
    }
    guard.disarm();
    Ok(())
}

pub fn extract_audio(ffmpeg: &Path, src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    let guard = crate::output_guard::OutputGuard::new(output);

    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-i")
        .arg(src)
        .arg("-vn")
        .arg("-c:a")
        .arg("libmp3lame")
        .arg("-q:a")
        .arg("2")
        .arg("-y")
        .arg(output);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let status = cmd
        .status()
        .map_err(|e| ConvertError::IoError(format!("Failed to spawn ffmpeg: {}", e)))?;

    if !status.success() {
        return Err(ConvertError::ConverterError("FFmpeg audio extraction failed".to_string()));
    }
    guard.disarm();
    Ok(())
}

pub fn compress_video(ffmpeg: &Path, src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    let guard = crate::output_guard::OutputGuard::new(output);

    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-i")
        .arg(src)
        .arg("-vcodec")
        .arg("libx264")
        .arg("-crf")
        .arg("28")
        .arg("-preset")
        .arg("fast")
        .arg("-y")
        .arg(output);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let status = cmd
        .status()
        .map_err(|e| ConvertError::IoError(format!("Failed to spawn ffmpeg: {}", e)))?;

    if !status.success() {
        return Err(ConvertError::ConverterError("FFmpeg video compression failed".to_string()));
    }
    guard.disarm();
    Ok(())
}

pub fn convert_audio(
    ffmpeg: &Path,
    src: &Path,
    output: &Path,
    target_format: &str,
) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-i").arg(src).arg("-vn");

    match target_format.to_ascii_lowercase().as_str() {
        "mp3" => {
            cmd.arg("-c:a").arg("libmp3lame").arg("-q:a").arg("2");
        }
        "wav" => {
            cmd.arg("-c:a").arg("pcm_s16le");
        }
        "ogg" => {
            cmd.arg("-c:a").arg("libvorbis").arg("-q:a").arg("4");
        }
        "aac" => {
            cmd.arg("-c:a").arg("aac").arg("-b:a").arg("192k");
        }
        "flac" => {
            cmd.arg("-c:a").arg("flac");
        }
        "m4a" => {
            cmd.arg("-c:a").arg("aac").arg("-b:a").arg("192k");
        }
        _ => {
            cmd.arg("-c:a").arg("copy");
        }
    }

    cmd.arg("-y").arg(output);

    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let status = cmd
        .status()
        .map_err(|e| ConvertError::IoError(format!("Failed to spawn ffmpeg: {}", e)))?;

    if !status.success() {
        return Err(ConvertError::ConverterError(format!(
            "FFmpeg audio conversion to {} failed",
            target_format
        )));
    }
    Ok(())
}
