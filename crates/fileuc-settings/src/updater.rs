use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

pub const GITHUB_REPO: &str = "Venkateshwar-T/fileuc";

static UPDATE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

struct UpdateLockGuard;
impl Drop for UpdateLockGuard {
    fn drop(&mut self) {
        UPDATE_IN_PROGRESS.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum UpdatePriority {
    #[default]
    Standard,
    Blocking,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingUpdate {
    pub version: String,
    pub release_url: String,
    pub release_name: Option<String>,
    pub download_url: Option<String>,
    #[serde(default)]
    pub priority: UpdatePriority,
    #[serde(default)]
    pub release_notes: Option<String>,
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
    pub body: Option<String>,
    #[serde(default)]
    pub assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
}

fn latest_release_api_url() -> String {
    format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest")
}

fn allowed_download_prefix() -> String {
    format!("https://github.com/{GITHUB_REPO}/releases/download/")
}

pub fn is_trusted_download_url(url: &str) -> bool {
    url.starts_with(&allowed_download_prefix()) && !url.contains("..")
}

pub fn updates_staging_dir() -> PathBuf {
    let local_app_data = std::env::var("LOCALAPPDATA")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    local_app_data.join("FileUC").join("updates")
}

/// Removes leftover downloaded update installers from previous sessions.
pub fn cleanup_old_updates() {
    if UPDATE_IN_PROGRESS.load(Ordering::SeqCst) {
        return;
    }
    let dir = updates_staging_dir();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

pub fn clean_version_label(tag: &str) -> String {
    let mut ver = tag.trim().trim_start_matches(|c| c == 'v' || c == 'V').to_string();
    let lower = ver.to_lowercase();
    for suffix in ["-critical", "-blocking", "-force", "+critical", "+blocking"] {
        if let Some(pos) = lower.find(suffix) {
            ver.truncate(pos);
            break;
        }
    }
    ver
}

pub fn detect_update_priority(
    tag_name: &str,
    release_name: Option<&str>,
    body: Option<&str>,
) -> UpdatePriority {
    let tag_lower = tag_name.to_lowercase();
    if tag_lower.contains("-critical")
        || tag_lower.contains("-blocking")
        || tag_lower.contains("-force")
        || tag_lower.contains("+critical")
        || tag_lower.contains("+blocking")
    {
        return UpdatePriority::Blocking;
    }

    let has_marker = |text: &str| {
        let lower = text.to_lowercase();
        lower.contains("[critical]")
            || lower.contains("[blocking]")
            || lower.contains("[force-update]")
    };

    if release_name.map(has_marker).unwrap_or(false) || body.map(has_marker).unwrap_or(false) {
        return UpdatePriority::Blocking;
    }

    UpdatePriority::Standard
}

fn summarize_release_notes(body: Option<&str>) -> Option<String> {
    let raw = body?;
    let cleaned_lines: Vec<String> = raw
        .lines()
        .map(|l| l.trim())
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with('#')
                && !l.eq_ignore_ascii_case("[critical]")
                && !l.eq_ignore_ascii_case("[blocking]")
        })
        .take(2)
        .map(|l| {
            l.trim_start_matches(['-', '*', ' '])
                .replace("[critical]", "")
                .replace("[blocking]", "")
                .replace("[CRITICAL]", "")
                .replace("[BLOCKING]", "")
                .trim()
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect();

    if cleaned_lines.is_empty() {
        None
    } else {
        Some(cleaned_lines.join(" "))
    }
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
    let api_url = latest_release_api_url();
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new("curl.exe");
    cmd.args([
        "-s",
        "-f",
        "-m",
        "8",
        "--connect-timeout",
        "5",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--tlsv1.2",
        "-H",
        "User-Agent: FileUC-App",
        &api_url,
    ]);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let output = cmd.output().map_err(|e| format!("Update check error: {e}"))?;
    if !output.status.success() {
        return Err("Network unavailable or offline".to_string());
    }

    let body = String::from_utf8_lossy(&output.stdout);
    if body.trim().is_empty() {
        return Err("Empty response from update server".to_string());
    }

    let release: GithubRelease = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse release metadata: {e}"))?;

    Ok(release)
}

pub fn check_for_updates_internal(_force: bool) -> UpdateCheckResponse {
    let current_pkg_ver = env!("CARGO_PKG_VERSION");
    let store = crate::store::SettingsStore::default();
    let mut settings = store.load();

    // Always query remote releases even on startup so newer/critical versions
    // aren't hidden behind an older cached pending_update.
    // Offline fallback restores valid cached pending_update if the check fails.
    match fetch_latest_github_release() {
        Ok(release) => {
            let remote_ver = clean_version_label(&release.tag_name);
            if is_newer_version(&remote_ver, current_pkg_ver) {
                let priority = detect_update_priority(
                    &release.tag_name,
                    release.name.as_deref(),
                    release.body.as_deref(),
                );
                let release_notes = summarize_release_notes(release.body.as_deref());

                // Find executable installer asset if available
                let download_url = release
                    .assets
                    .iter()
                    .find(|a| a.name.ends_with(".exe") || a.name.contains("Setup"))
                    .map(|a| a.browser_download_url.clone())
                    .or_else(|| Some(release.html_url.clone()));

                let pending = PendingUpdate {
                    version: remote_ver,
                    release_url: release.html_url,
                    release_name: release.name,
                    download_url,
                    priority,
                    release_notes,
                };

                if settings.pending_update.as_ref() != Some(&pending) {
                    settings.pending_update = Some(pending.clone());
                    let _ = store.save(&settings);
                }

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
            // If offline, fall back to cached pending update if still newer
            if let Some(ref pending) = settings.pending_update {
                if is_newer_version(&pending.version, current_pkg_ver) {
                    return UpdateCheckResponse {
                        status: "update_available".to_string(),
                        current_version: current_pkg_ver.to_string(),
                        update: Some(pending.clone()),
                        message: Some("Offline, cached update available".to_string()),
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

fn verify_pe_header(path: &std::path::Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 2];
    file.read_exact(&mut magic).is_ok() && magic == *b"MZ"
}

pub fn download_and_install_update<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    download_url: String,
) -> Result<(), String> {
    use tauri::Emitter;

    // Security: Enforce strict HTTPS + pinned owner/repo release allowlisting
    if !is_trusted_download_url(&download_url) {
        return Err("Untrusted update source.".to_string());
    }

    // If download url is not an installer, fallback to browser
    let is_direct_installer = download_url.ends_with(".exe") || download_url.contains("Setup");
    if !is_direct_installer {
        let _ = open_url_in_browser(&download_url);
        return Ok(());
    }

    // Concurrency guard: prevent duplicate simultaneous update downloads
    if UPDATE_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("An update download is already in progress.".to_string());
    }

    std::thread::spawn(move || {
        let _lock = UpdateLockGuard;

        let emit_err = |msg: &str| {
            let _ = app_handle.emit(
                "update-progress",
                UpdateProgressPayload {
                    progress: 0.0,
                    text: msg.to_string(),
                    done: true,
                    error: Some(msg.to_string()),
                },
            );
        };

        let _ = app_handle.emit(
            "update-progress",
            UpdateProgressPayload {
                progress: 0.05,
                text: "Downloading...".to_string(),
                done: false,
                error: None,
            },
        );

        let updates_dir = updates_staging_dir();
        if let Err(e) = std::fs::create_dir_all(&updates_dir) {
            emit_err(&format!("Failed to prepare update directory: {e}"));
            return;
        }

        let unique_id = std::process::id();
        let temp_download = updates_dir.join(format!("FileUC-Setup-Update-{unique_id}.tmp"));
        let target_installer = updates_dir.join(format!("FileUC-Setup-Update-{unique_id}.exe"));

        let _ = std::fs::remove_file(&temp_download);
        let _ = std::fs::remove_file(&target_installer);

        // 1. Query Content-Length via curl.exe HEAD request (-I) with strict HTTPS
        let mut head_cmd = std::process::Command::new("curl.exe");
        head_cmd.args([
            "-s",
            "-I",
            "-L",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--tlsv1.2",
            "-m",
            "10",
            "--connect-timeout",
            "8",
            &download_url,
        ]);
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
                    lower
                        .strip_prefix("content-length:")
                        .and_then(|v| v.trim().parse::<u64>().ok())
                })
                .last()
        } else {
            None
        };

        // 2. Start curl download with HTTPS enforcement and stall timeouts
        let mut dl_cmd = std::process::Command::new("curl.exe");
        dl_cmd.args([
            "-L",
            "-f",
            "-s",
            "-S",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--tlsv1.2",
            "--connect-timeout",
            "15",
            "--speed-limit",
            "1024",
            "--speed-time",
            "30",
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
                emit_err(&format!("Failed to start download: {e}"));
                return;
            }
        };

        // 3. Monitor download progress by sampling file size
        while let Ok(None) = child.try_wait() {
            if let Ok(meta) = std::fs::metadata(&temp_download) {
                let current_bytes = meta.len();
                let pct = match total_bytes {
                    Some(total) if total > 0 => (current_bytes as f32 / total as f32).clamp(0.05, 0.98),
                    _ => 0.5,
                };
                let _ = app_handle.emit(
                    "update-progress",
                    UpdateProgressPayload {
                        progress: pct,
                        text: "Downloading...".to_string(),
                        done: false,
                        error: None,
                    },
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        }

        let status = match child.wait() {
            Ok(s) => s,
            Err(e) => {
                let _ = std::fs::remove_file(&temp_download);
                emit_err(&e.to_string());
                return;
            }
        };

        if !status.success() {
            let _ = std::fs::remove_file(&temp_download);
            emit_err("Download failed or timed out");
            return;
        }

        // Strict integrity checks: file must exist, exceed minimum size, match Content-Length, and have MZ PE header
        let meta = match std::fs::metadata(&temp_download) {
            Ok(m) => m,
            Err(_) => {
                let _ = std::fs::remove_file(&temp_download);
                emit_err("Downloaded file missing");
                return;
            }
        };

        if meta.len() < 5_000_000 {
            let _ = std::fs::remove_file(&temp_download);
            emit_err("Downloaded update file is incomplete");
            return;
        }

        if let Some(expected) = total_bytes {
            if expected > 0 && meta.len() != expected {
                let _ = std::fs::remove_file(&temp_download);
                emit_err("Downloaded update file size mismatch");
                return;
            }
        }

        if !verify_pe_header(&temp_download) {
            let _ = std::fs::remove_file(&temp_download);
            emit_err("Downloaded update file failed executable verification");
            return;
        }

        let _ = std::fs::remove_file(&target_installer);
        if let Err(e) = std::fs::rename(&temp_download, &target_installer) {
            let _ = std::fs::remove_file(&temp_download);
            emit_err(&format!("Failed to prepare installer: {e}"));
            return;
        }

        // 4. Emit 100% completion and restart status
        let _ = app_handle.emit(
            "update-progress",
            UpdateProgressPayload {
                progress: 1.0,
                text: "Installing update...".to_string(),
                done: true,
                error: None,
            },
        );

        std::thread::sleep(std::time::Duration::from_millis(500));

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
            } else {
                emit_err("Failed to launch installer");
            }
        }
    });

    Ok(())
}

pub fn open_url_in_browser(url: &str) -> Result<(), String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Invalid URL protocol: Only HTTP/HTTPS URLs are permitted.".to_string());
    }

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
                return Err(format!("ShellExecuteW failed to open URL (error code: {})", res.0 as isize));
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
        assert_eq!(parse_semver("v2.0.0-critical"), (2, 0, 0));
        assert_eq!(parse_semver("0.1.1+build.123"), (0, 1, 1));
    }

    #[test]
    fn test_clean_version_label() {
        assert_eq!(clean_version_label("v0.1.6"), "0.1.6");
        assert_eq!(clean_version_label("v0.1.6-critical"), "0.1.6");
        assert_eq!(clean_version_label("V0.1.6-blocking"), "0.1.6");
        assert_eq!(clean_version_label("v0.2.0-beta.1"), "0.2.0-beta.1");
    }

    #[test]
    fn test_detect_update_priority() {
        assert_eq!(
            detect_update_priority("v0.1.6", None, None),
            UpdatePriority::Standard
        );
        assert_eq!(
            detect_update_priority("v0.1.6-critical", None, None),
            UpdatePriority::Blocking
        );
        assert_eq!(
            detect_update_priority("v0.1.6-blocking", None, None),
            UpdatePriority::Blocking
        );
        assert_eq!(
            detect_update_priority("v0.1.6", Some("Hotfix [CRITICAL]"), None),
            UpdatePriority::Blocking
        );
        assert_eq!(
            detect_update_priority("v0.1.6", None, Some("Fixes crash.\n[blocking]")),
            UpdatePriority::Blocking
        );
    }

    #[test]
    fn test_is_trusted_download_url() {
        assert!(is_trusted_download_url(
            "https://github.com/Venkateshwar-T/fileuc/releases/download/v0.1.6/FileUC-Setup.exe"
        ));
        assert!(!is_trusted_download_url(
            "https://github.com/Sharad-Chand/FileUC/releases/download/v0.1.6/FileUC-Setup.exe"
        ));
        assert!(!is_trusted_download_url(
            "https://objects.githubusercontent.com/arbitrary-repo/setup.exe"
        ));
        assert!(!is_trusted_download_url(
            "http://github.com/Venkateshwar-T/fileuc/releases/download/v0.1.6/FileUC-Setup.exe"
        ));
        assert!(!is_trusted_download_url(
            "https://github.com/Venkateshwar-T/fileuc/releases/download/../evil.exe"
        ));
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.2.0", "0.1.0"));
        assert!(is_newer_version("v0.2.0", "0.1.0"));
        assert!(is_newer_version("0.1.1", "0.1.0"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(is_newer_version("v0.1.6-critical", "0.1.5"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("v0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.0.9", "0.1.0"));
    }
}
