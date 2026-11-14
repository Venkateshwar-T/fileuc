//! High-resolution QR code PNG image exporter.

use std::path::PathBuf;

/// Resolves the user's Windows Desktop directory path.
pub fn get_desktop_dir() -> PathBuf {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::Shell::{SHGetKnownFolderPath, FOLDERID_Desktop, KNOWN_FOLDER_FLAG};
        if let Ok(pwstr) = SHGetKnownFolderPath(&FOLDERID_Desktop, KNOWN_FOLDER_FLAG(0), None) {
            let path_str = pwstr.to_string().unwrap_or_default();
            windows::Win32::System::Com::CoTaskMemFree(Some(pwstr.as_ptr() as *const _));
            if !path_str.is_empty() {
                let p = PathBuf::from(path_str);
                if p.exists() {
                    return p;
                }
            }
        }
    }

    if let Ok(profile) = std::env::var("USERPROFILE") {
        let desk = PathBuf::from(profile).join("Desktop");
        if desk.exists() {
            return desk;
        }
    }

    PathBuf::from(".")
}

/// Generates a crisp, high-resolution 512x512 PNG file of the QR code and saves it directly to the user's Desktop.
pub fn save_qr_to_desktop(matrix: &[Vec<bool>], prefix: &str) -> Option<PathBuf> {
    let desktop = get_desktop_dir();
    let n = matrix.len();
    if n == 0 {
        return None;
    }

    // High-resolution 512x512 export with 4-module optical quiet zone
    let target_size = 512u32;
    let quiet_zone = 4.0f32;
    let total_modules = (n as f32) + quiet_zone * 2.0;
    let module_size = (target_size as f32) / total_modules;
    let margin = quiet_zone * module_size;

    let mut pixmap = tiny_skia::Pixmap::new(target_size, target_size)?;
    pixmap.fill(tiny_skia::Color::WHITE);

    let mut qr_pb = tiny_skia::PathBuilder::new();
    for r in 0..n {
        for c in 0..n {
            if matrix[r][c] {
                let mx = margin + (c as f32) * module_size;
                let my = margin + (r as f32) * module_size;
                if let Some(rect) = tiny_skia::Rect::from_xywh(mx, my, module_size + 0.1, module_size + 0.1) {
                    qr_pb.push_rect(rect);
                }
            }
        }
    }

    if let Some(path) = qr_pb.finish() {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(0, 0, 0, 255);
        paint.anti_alias = false; // Sharp square pixel corners for optimal scanning
        pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let clean_prefix = prefix.replace(|c: char| !c.is_alphanumeric() && c != '_', "");
    let filename = format!("QRCode_{}_{}.png", clean_prefix, timestamp);
    let out_path = desktop.join(filename);

    pixmap.save_png(&out_path).ok()?;
    Some(out_path)
}
