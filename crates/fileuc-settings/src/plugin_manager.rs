use std::path::PathBuf;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[allow(dead_code)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn plugins_dir() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local_app_data).join("FileUC").join("plugins")
    } else {
        PathBuf::from("plugins")
    }
}

pub fn is_docx_installed() -> bool {
    let docx_py = plugins_dir().join("pdf2docx").join("Scripts").join("python.exe");
    docx_py.exists()
}

pub fn is_ffmpeg_installed() -> bool {
    let ffmpeg_exe = plugins_dir().join("ffmpeg").join("ffmpeg.exe");
    if ffmpeg_exe.exists() {
        return true;
    }
    let candidates = [
        PathBuf::from(r"C:\Program Files\FileUC\engines\ffmpeg\ffmpeg.exe"),
        PathBuf::from(r"C:\Program Files\ffmpeg\bin\ffmpeg.exe"),
        PathBuf::from(r"C:\ffmpeg\bin\ffmpeg.exe"),
    ];
    for c in &candidates {
        if c.exists() {
            return true;
        }
    }
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-version");
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    if let Ok(out) = cmd.output() {
        if out.status.success() {
            return true;
        }
    }
    false
}

pub fn uninstall_docx() -> std::io::Result<()> {
    let docx_dir = plugins_dir().join("pdf2docx");
    if docx_dir.exists() {
        std::fs::remove_dir_all(&docx_dir)?;
    }
    Ok(())
}

pub fn uninstall_ffmpeg() -> std::io::Result<()> {
    let ffmpeg_dir = plugins_dir().join("ffmpeg");
    if ffmpeg_dir.exists() {
        std::fs::remove_dir_all(&ffmpeg_dir)?;
    }
    Ok(())
}

fn find_uv() -> Option<PathBuf> {
    // 1. Check %LOCALAPPDATA%\FileUC\bin\uv.exe
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let local_uv = PathBuf::from(&local_app_data).join("FileUC").join("bin").join("uv.exe");
        if local_uv.exists() {
            return Some(local_uv);
        }
    }
    // 2. Check %USERPROFILE%\.cargo\bin\uv.exe or %USERPROFILE%\.local\bin\uv.exe
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let cargo_uv = PathBuf::from(&user_profile).join(".cargo").join("bin").join("uv.exe");
        if cargo_uv.exists() {
            return Some(cargo_uv);
        }
        let local_uv = PathBuf::from(&user_profile).join(".local").join("bin").join("uv.exe");
        if local_uv.exists() {
            return Some(local_uv);
        }
    }
    // 3. Check system PATH
    let mut cmd = Command::new("uv");
    cmd.arg("--version");
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    if let Ok(out) = cmd.output() {
        if out.status.success() {
            return Some(PathBuf::from("uv"));
        }
    }
    None
}

pub fn install_docx<F>(progress_cb: F) -> Result<(), String>
where
    F: Fn(f32, &str) + Send + 'static,
{
    let target_dir = plugins_dir().join("pdf2docx");
    let _ = std::fs::create_dir_all(&target_dir);

    progress_cb(0.10, "Locating package manager...");

    let uv_bin = find_uv().ok_or_else(|| {
        "uv package manager not found. Please ensure uv is available on system.".to_string()
    })?;

    progress_cb(0.20, "Creating isolated virtual environment...");

    // 1. Create venv
    let mut venv_cmd = Command::new(&uv_bin);
    venv_cmd.args(["venv", &target_dir.to_string_lossy(), "--python", "3.13"]);
    #[cfg(windows)]
    {
        venv_cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let status = venv_cmd
        .status()
        .map_err(|e| format!("Failed to create virtual environment: {}", e))?;

    if !status.success() {
        return Err("Failed to create Python virtual environment".to_string());
    }

    progress_cb(0.40, "Downloading Office & Document Processing engine (120 MB)...");

    let python_exe = target_dir.join("Scripts").join("python.exe");

    // 2. Install pdf2docx into venv
    let mut pip_cmd = Command::new(&uv_bin);
    pip_cmd.args([
        "pip",
        "install",
        "--python",
        &python_exe.to_string_lossy(),
        "pdf2docx",
        "openpyxl",
        "reportlab",
        "pdfplumber",
        "rapidocr-onnxruntime",
    ]);
    #[cfg(windows)]
    {
        pip_cmd.creation_flags(CREATE_NO_WINDOW);
    }

    progress_cb(0.70, "Installing Word, Excel, PDF & OCR reconstruction engines...");

    let pip_status = pip_cmd
        .status()
        .map_err(|e| format!("Failed to install dependencies: {}", e))?;

    if !pip_status.success() {
        return Err("Failed to install packages into virtual environment".to_string());
    }

    progress_cb(0.95, "Validating installation...");

    // 3. Validate import
    let mut check_cmd = Command::new(&python_exe);
    check_cmd.args([
        "-c",
        "import pdf2docx; import openpyxl; import reportlab; import pdfplumber; import rapidocr_onnxruntime",
    ]);
    #[cfg(windows)]
    {
        check_cmd.creation_flags(CREATE_NO_WINDOW);
    }

    if let Ok(out) = check_cmd.output() {
        if out.status.success() {
            progress_cb(1.0, "Ready");
            return Ok(());
        }
    }

    Err("pdf2docx validation import failed".to_string())
}

pub fn install_ffmpeg<F>(progress_cb: F) -> Result<(), String>
where
    F: Fn(f32, &str) + Send + 'static,
{
    let target_dir = plugins_dir().join("ffmpeg");
    let _ = std::fs::create_dir_all(&target_dir);

    progress_cb(0.10, "Initializing download of FFmpeg...");
    let zip_path = plugins_dir().join("ffmpeg.zip");

    let download_url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
    progress_cb(0.30, "Downloading FFmpeg Media Engine (~35 MB)...");

    let mut curl_cmd = Command::new("curl");
    curl_cmd.args(["-L", "-f", "-o", &zip_path.to_string_lossy(), download_url]);
    #[cfg(windows)]
    {
        curl_cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let curl_status = curl_cmd.status();
    let download_ok = match curl_status {
        Ok(s) => s.success(),
        Err(_) => false,
    };

    if !download_ok {
        progress_cb(0.40, "Downloading via PowerShell...");
        let ps_script = format!(
            "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; (New-Object System.Net.WebClient).DownloadFile('{}', '{}')",
            download_url,
            zip_path.to_string_lossy().replace('\\', "\\\\")
        );
        let mut ps_cmd = Command::new("powershell");
        ps_cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps_script]);
        #[cfg(windows)]
        {
            ps_cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let ps_status = ps_cmd.status().map_err(|e| format!("Download failed: {}", e))?;
        if !ps_status.success() {
            let _ = std::fs::remove_file(&zip_path);
            return Err("Failed to download FFmpeg archive".to_string());
        }
    }

    progress_cb(0.75, "Extracting FFmpeg binaries...");

    let mut tar_cmd = Command::new("tar");
    tar_cmd.args(["-xf", &zip_path.to_string_lossy(), "-C", &target_dir.to_string_lossy()]);
    #[cfg(windows)]
    {
        tar_cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let tar_ok = match tar_cmd.status() {
        Ok(s) => s.success(),
        Err(_) => false,
    };

    if !tar_ok {
        let ps_extract = format!(
            "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
            zip_path.to_string_lossy().replace('\\', "\\\\"),
            target_dir.to_string_lossy().replace('\\', "\\\\")
        );
        let mut ps_cmd = Command::new("powershell");
        ps_cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps_extract]);
        #[cfg(windows)]
        {
            ps_cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let _ = ps_cmd.status();
    }

    let _ = std::fs::remove_file(&zip_path);

    progress_cb(0.90, "Locating binary...");
    let direct_exe = target_dir.join("ffmpeg.exe");
    if !direct_exe.exists() {
        if let Ok(entries) = std::fs::read_dir(&target_dir) {
            for entry in entries.flatten() {
                let bin_candidate = entry.path().join("bin").join("ffmpeg.exe");
                if bin_candidate.exists() {
                    let _ = std::fs::copy(&bin_candidate, &direct_exe);
                    break;
                }
            }
        }
    }

    // Clean up residual ~100MB unzipped archive directories now that ffmpeg.exe is in place
    if direct_exe.exists() {
        if let Ok(entries) = std::fs::read_dir(&target_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let _ = std::fs::remove_dir_all(&p);
                }
            }
        }
    }

    if is_ffmpeg_installed() {
        progress_cb(1.0, "Ready");
        Ok(())
    } else {
        Err("FFmpeg extraction completed but ffmpeg.exe was not found".to_string())
    }
}
