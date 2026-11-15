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
