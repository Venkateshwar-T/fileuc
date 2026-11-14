//! Wi-Fi Network Name (SSID) detection for Windows.

use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Queries the connected Wi-Fi Network Name (SSID) using Windows NetConnectionProfile.
pub fn detect_wifi_ssid() -> Option<String> {
    // 1. Direct query for Wi-Fi interface profile
    if let Ok(output) = Command::new("powershell")
        .args(&[
            "-NoProfile",
            "-Command",
            "(Get-NetConnectionProfile -InterfaceAlias 'Wi-Fi').Name",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        if output.status.success() {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }

    // 2. Fallback query for any active connection profile
    if let Ok(output) = Command::new("powershell")
        .args(&[
            "-NoProfile",
            "-Command",
            "(Get-NetConnectionProfile | Select-Object -ExpandProperty Name -First 1)",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        if output.status.success() {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }

    None
}
