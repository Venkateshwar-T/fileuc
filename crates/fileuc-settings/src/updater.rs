use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingUpdate {
    pub version: String,
    pub release_url: String,
    pub release_name: Option<String>,
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResponse {
    pub status: String, // "update_available", "up_to_date", "offline", "error"
    pub current_version: String,
    pub update: Option<PendingUpdate>,
    pub message: Option<String>,
}

#[derive(Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
}

pub fn parse_semver(s: &str) -> (u32, u32, u32) {
    let clean = s.trim().trim_start_matches(|c| c == 'v' || c == 'V');
    let base = clean.split(['-', '+']).next().unwrap_or(clean);
    let parts: Vec<&str> = base.split('.').collect();
    let major = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

pub fn is_newer_version(remote: &str, current: &str) -> bool {
    let r = parse_semver(remote);
    let c = parse_semver(current);
    r > c
}

pub fn fetch_latest_github_release() -> Result<GithubRelease, String> {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new("curl.exe");
    cmd.args([
        "-s",
        "-m",
        "5",
        "-H",
        "User-Agent: FileUC-App",
        "https://api.github.com/repos/Venkateshwar-T/fileuc/releases/latest",
    ]);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let output = cmd.output().map_err(|e| format!("curl execution failed: {e}"))?;
    if !output.status.success() {
        return Err("curl returned non-zero exit code (network error or offline)".to_string());
    }

    let body = String::from_utf8_lossy(&output.stdout);
    if body.trim().is_empty() {
        return Err("Empty response from GitHub".to_string());
    }

    let release: GithubRelease = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse GitHub release JSON: {e}"))?;

    Ok(release)
}

pub fn check_for_updates_internal(force: bool) -> UpdateCheckResponse {
    let current_pkg_ver = env!("CARGO_PKG_VERSION");
    let store = crate::store::SettingsStore::default();
    let mut settings = store.load();

    // If not forced and we already have a valid pending update stored, return it immediately without hitting GitHub!
    if !force {
        if let Some(ref pending) = settings.pending_update {
            if is_newer_version(&pending.version, current_pkg_ver) {
                return UpdateCheckResponse {
                    status: "update_available".to_string(),
                    current_version: current_pkg_ver.to_string(),
                    update: Some(pending.clone()),
                    message: Some("Cached update available".to_string()),
                };
            }
        }
    }

    // Query GitHub releases API
    match fetch_latest_github_release() {
        Ok(release) => {
            let remote_ver = release.tag_name.trim().trim_start_matches(|c| c == 'v' || c == 'V').to_string();
            if is_newer_version(&remote_ver, current_pkg_ver) {
                // Find executable installer asset if available
                let download_url = release.assets.iter()
                    .find(|a| a.name.ends_with(".exe") || a.name.contains("Setup"))
                    .map(|a| a.browser_download_url.clone())
                    .or_else(|| Some(release.html_url.clone()));

                let pending = PendingUpdate {
                    version: remote_ver,
                    release_url: release.html_url,
                    release_name: release.name,
                    download_url,
                };

                // Save to settings permanently
                settings.pending_update = Some(pending.clone());
                let _ = store.save(&settings);

                UpdateCheckResponse {
                    status: "update_available".to_string(),
                    current_version: current_pkg_ver.to_string(),
                    update: Some(pending),
                    message: Some("New version available".to_string()),
                }
            } else {
                // Current version is up to date or newer
                if settings.pending_update.is_some() {
                    settings.pending_update = None;
                    let _ = store.save(&settings);
                }

                UpdateCheckResponse {
                    status: "up_to_date".to_string(),
                    current_version: current_pkg_ver.to_string(),
                    update: None,
                    message: Some("You have the latest version".to_string()),
                }
            }
        }
        Err(e) => {
            // If offline, check if we still have a cached pending update
            if let Some(ref pending) = settings.pending_update {
                if is_newer_version(&pending.version, current_pkg_ver) {
                    return UpdateCheckResponse {
                        status: "update_available".to_string(),
                        current_version: current_pkg_ver.to_string(),
                        update: Some(pending.clone()),
                        message: Some("Offline, but cached update is available".to_string()),
                    };
                }
            }

            UpdateCheckResponse {
                status: "offline".to_string(),
                current_version: current_pkg_ver.to_string(),
                update: None,
                message: Some(e),
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProgressPayload {
    pub progress: f32, // 0.0 to 1.0
    pub text: String,
    pub done: bool,
    pub error: Option<String>,
}

pub fn download_and_install_update<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    download_url: String,
) -> Result<(), String> {
    use tauri::Emitter;

    // If download url is not an installer, fallback to browser
    let is_direct_installer = download_url.ends_with(".exe") || download_url.contains("Setup");
    if !is_direct_installer {
        let _ = open_url_in_browser(&download_url);
        return Ok(());
    }

    std::thread::spawn(move || {
        let _ = app_handle.emit(
            "update-progress",
            UpdateProgressPayload {
                progress: 0.05,
                text: "Connecting to GitHub server...".to_string(),
                done: false,
                error: None,
            },
        );

        let temp_dir = std::env::temp_dir();
        let target_installer = temp_dir.join("FileUC-Setup-Update.exe");
        let temp_download = temp_dir.join("FileUC-Setup-Update.tmp");

        let _ = std::fs::remove_file(&temp_download);

        // 1. Query Content-Length via curl.exe HEAD request (-I)
        let mut head_cmd = std::process::Command::new("curl.exe");
        head_cmd.args(["-s", "-I", "-L", "-m", "10", &download_url]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            head_cmd.creation_flags(0x08000000);
        }
        let total_bytes: Option<u64> = if let Ok(output) = head_cmd.output() {
            let s = String::from_utf8_lossy(&output.stdout);
            s.lines()
                .filter_map(|line| {
                    let lower = line.to_lowercase();
                    if lower.starts_with("content-length:") {
                        lower.strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<u64>().ok())
                    } else {
                        None
                    }
                })
                .last()
        } else {
            None
        };

        // 2. Start curl download
        let mut dl_cmd = std::process::Command::new("curl.exe");
        dl_cmd.args([
            "-L",
            "-f",
            "-s",
            "-S",
            "-o",
            temp_download.to_string_lossy().as_ref(),
            &download_url,
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            dl_cmd.creation_flags(0x08000000);
        }

        let mut child = match dl_cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = app_handle.emit(
                    "update-progress",
                    UpdateProgressPayload {
                        progress: 0.0,
                        text: format!("Download error: {e}"),
                        done: true,
                        error: Some(format!("Failed to invoke curl: {e}")),
                    },
                );
                return;
            }
        };

        // 3. Monitor download progress by sampling file size
        while let Ok(None) = child.try_wait() {
            if let Ok(meta) = std::fs::metadata(&temp_download) {
                let current_bytes = meta.len();
                let current_mb = current_bytes as f64 / 1_048_576.0;
                if let Some(total) = total_bytes {
                    if total > 0 {
                        let pct = (current_bytes as f32 / total as f32).min(0.98);
                        let total_mb = total as f64 / 1_048_576.0;
                        let text = format!("Downloading update: {:.1} MB / {:.1} MB ({:.0}%)", current_mb, total_mb, pct * 100.0);
                        let _ = app_handle.emit(
                            "update-progress",
                            UpdateProgressPayload {
                                progress: pct,
                                text,
                                done: false,
                                error: None,
                            },
                        );
                    }
                } else {
                    let text = format!("Downloading update: {:.1} MB", current_mb);
                    let _ = app_handle.emit(
                        "update-progress",
                        UpdateProgressPayload {
                            progress: 0.5,
                            text,
                            done: false,
                            error: None,
                        },
                    );
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        }

        let status = match child.wait() {
            Ok(s) => s,
            Err(e) => {
                let _ = std::fs::remove_file(&temp_download);
                let _ = app_handle.emit(
                    "update-progress",
                    UpdateProgressPayload {
                        progress: 0.0,
                        text: format!("Download error: {e}"),
                        done: true,
                        error: Some(e.to_string()),
                    },
                );
                return;
            }
        };

        if !status.success() {
            let _ = std::fs::remove_file(&temp_download);
            let _ = app_handle.emit(
                "update-progress",
                UpdateProgressPayload {
                    progress: 0.0,
                    text: "Download failed or was interrupted".to_string(),
                    done: true,
                    error: Some("Download exited with error".to_string()),
                },
            );
            return;
        }

        // Verify file exists and is plausible installer size (>5MB)
        if let Ok(meta) = std::fs::metadata(&temp_download) {
            if meta.len() < 5_000_000 {
                let _ = std::fs::remove_file(&temp_download);
                let _ = app_handle.emit(
                    "update-progress",
                    UpdateProgressPayload {
                        progress: 0.0,
                        text: "Downloaded file is corrupted or incomplete".to_string(),
                        done: true,
                        error: Some("File size too small".to_string()),
                    },
                );
                return;
            }
        }

        let _ = std::fs::remove_file(&target_installer);
        if let Err(e) = std::fs::rename(&temp_download, &target_installer) {
            let _ = app_handle.emit(
                "update-progress",
                UpdateProgressPayload {
                    progress: 0.0,
                    text: format!("Failed to save installer: {e}"),
                    done: true,
                    error: Some(e.to_string()),
                },
            );
            return;
        }

        // 4. Emit 100% completion and restart status
        let _ = app_handle.emit(
            "update-progress",
            UpdateProgressPayload {
                progress: 1.0,
                text: "Restarting and applying update...".to_string(),
                done: true,
                error: None,
            },
        );

        std::thread::sleep(std::time::Duration::from_millis(600));

        // 5. Launch NSIS installer in silent mode (/S) and exit current process
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let mut inst = std::process::Command::new(&target_installer);
            inst.arg("/S");
            inst.creation_flags(0x08000000);
            if inst.spawn().is_ok() {
                std::thread::sleep(std::time::Duration::from_millis(300));
                std::process::exit(0);
            }
        }
    });

    Ok(())
}

pub fn open_url_in_browser(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::core::HSTRING;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::Win32::Foundation::HWND;

        unsafe {
            let res = ShellExecuteW(
                HWND(std::ptr::null_mut()),
                &HSTRING::from("open"),
                &HSTRING::from(url),
                &HSTRING::default(),
                &HSTRING::default(),
                SW_SHOWNORMAL,
            );
            if (res.0 as isize) <= 32 {
                use std::os::windows::process::CommandExt;
                let mut cmd = std::process::Command::new("cmd");
                cmd.args(["/c", "start", "", url]);
                cmd.creation_flags(0x08000000);
                cmd.spawn().map_err(|e| format!("Failed to open URL: {e}"))?;
            }
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let mut cmd = std::process::Command::new("open");
        if cmd.arg(url).spawn().is_err() {
            let mut cmd2 = std::process::Command::new("xdg-open");
            cmd2.arg(url).spawn().map_err(|e| format!("Failed to open URL: {e}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_semver() {
        assert_eq!(parse_semver("0.1.0"), (0, 1, 0));
        assert_eq!(parse_semver("v0.1.0"), (0, 1, 0));
        assert_eq!(parse_semver("V1.2.3"), (1, 2, 3));
        assert_eq!(parse_semver("v2.0.0-beta.1"), (2, 0, 0));
        assert_eq!(parse_semver("0.1.1+build.123"), (0, 1, 1));
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.2.0", "0.1.0"));
        assert!(is_newer_version("v0.2.0", "0.1.0"));
        assert!(is_newer_version("0.1.1", "0.1.0"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("v0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.0.9", "0.1.0"));
    }
}
