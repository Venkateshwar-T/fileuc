use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardKind {
    Files,
    Image,
    Text,
    Link,
}

#[derive(Debug, Clone)]
pub struct ClipboardPayload {
    pub kind: ClipboardKind,
    pub paths: Vec<PathBuf>,
    pub save_location: PathBuf,
    pub is_from_disk: bool,
    pub raw_text: Option<String>,
}

pub fn get_desktop_dir() -> PathBuf {
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let desk = PathBuf::from(&profile).join("Desktop");
        if desk.exists() {
            return desk;
        }
    }
    std::env::temp_dir()
}

pub fn is_url_string(s: &str) -> bool {
    let t = s.trim();
    (t.starts_with("http://") || t.starts_with("https://") || t.starts_with("ftp://") || t.starts_with("www."))
        && !t.contains('\n')
        && !t.contains(' ')
}

pub fn get_clipboard_payload() -> Option<ClipboardPayload> {
    unsafe {
        use windows::Win32::Foundation::HGLOBAL;
        use windows::Win32::System::DataExchange::{
            CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
        };
        use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

        if OpenClipboard(None).is_err() {
            return None;
        }

        // 1. Check CF_HDROP (15) - Copied files in Windows Explorer
        if IsClipboardFormatAvailable(15).is_ok() {
            if let Ok(handle) = GetClipboardData(15) {
                let hglobal = HGLOBAL(handle.0);
                let ptr = GlobalLock(hglobal);
                if !ptr.is_null() {
                    let total_bytes = GlobalSize(hglobal);
                    let slice = std::slice::from_raw_parts(ptr as *const u8, total_bytes);
                    let parsed = fileuc_hud::parse_hdrop_buffer(slice);
                    let _ = GlobalUnlock(hglobal);
                    if let Ok(paths) = parsed {
                        let valid: Vec<PathBuf> =
                            paths.into_iter().filter(|p| p.exists()).collect();
                        if !valid.is_empty() {
                            let _ = CloseClipboard();
                            let save_location = valid
                                .first()
                                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                                .unwrap_or_else(get_desktop_dir);
                            return Some(ClipboardPayload {
                                kind: ClipboardKind::Files,
                                paths: valid,
                                save_location,
                                is_from_disk: true,
                                raw_text: None,
                            });
                        }
                    }
                }
            }
        }

        // 2. Check CF_DIB (8) - Image copied to clipboard
        if IsClipboardFormatAvailable(8).is_ok() {
            if let Ok(handle) = GetClipboardData(8) {
                let hglobal = HGLOBAL(handle.0);
                let ptr = GlobalLock(hglobal);
                if !ptr.is_null() {
                    let total_bytes = GlobalSize(hglobal);
                    let slice = std::slice::from_raw_parts(ptr as *const u8, total_bytes);
                    let temp_dir = std::env::temp_dir().join("FileUC");
                    let _ = std::fs::create_dir_all(&temp_dir);
                    let temp_img = temp_dir.join("clipboard_image.png");
                    let saved = fileuc_convert::image::save_dib_as_png(slice, &temp_img);
                    let _ = GlobalUnlock(hglobal);
                    if saved.is_ok() {
                        let _ = CloseClipboard();
                        return Some(ClipboardPayload {
                            kind: ClipboardKind::Image,
                            paths: vec![temp_img],
                            save_location: get_desktop_dir(),
                            is_from_disk: false,
                            raw_text: None,
                        });
                    }
                }
            }
        }

        // 3. Check CF_UNICODETEXT (13) - Text or Link copied to clipboard
        if IsClipboardFormatAvailable(13).is_ok() {
            if let Ok(handle) = GetClipboardData(13) {
                let hglobal = HGLOBAL(handle.0);
                let ptr = GlobalLock(hglobal);
                if !ptr.is_null() {
                    let total_bytes = GlobalSize(hglobal);
                    let max_u16 = (total_bytes / 2) as usize;
                    let u16_ptr = ptr as *const u16;
                    let mut len = 0;
                    while len < max_u16 && *u16_ptr.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(u16_ptr, len);
                    let text = String::from_utf16_lossy(slice);
                    let _ = GlobalUnlock(hglobal);
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        // Check if text is an existing file path
                        let potential_path = PathBuf::from(trimmed);
                        if potential_path.exists() {
                            let _ = CloseClipboard();
                            let save_location = potential_path
                                .parent()
                                .map(|p| p.to_path_buf())
                                .unwrap_or_else(get_desktop_dir);
                            return Some(ClipboardPayload {
                                kind: ClipboardKind::Files,
                                paths: vec![potential_path],
                                save_location,
                                is_from_disk: true,
                                raw_text: None,
                            });
                        }

                        let temp_dir = std::env::temp_dir().join("FileUC");
                        let _ = std::fs::create_dir_all(&temp_dir);

                        if is_url_string(trimmed) {
                            let temp_link = temp_dir.join("clipboard_link.txt");
                            if std::fs::write(&temp_link, trimmed.as_bytes()).is_ok() {
                                let _ = CloseClipboard();
                                return Some(ClipboardPayload {
                                    kind: ClipboardKind::Link,
                                    paths: vec![temp_link],
                                    save_location: get_desktop_dir(),
                                    is_from_disk: false,
                                    raw_text: Some(trimmed.to_string()),
                                });
                            }
                        } else {
                            let temp_txt = temp_dir.join("clipboard_text.txt");
                            if std::fs::write(&temp_txt, trimmed.as_bytes()).is_ok() {
                                let _ = CloseClipboard();
                                return Some(ClipboardPayload {
                                    kind: ClipboardKind::Text,
                                    paths: vec![temp_txt],
                                    save_location: get_desktop_dir(),
                                    is_from_disk: false,
                                    raw_text: Some(trimmed.to_string()),
                                });
                            }
                        }
                    }
                }
            }
        }

        let _ = CloseClipboard();
        None
    }
}

/// Securely purges transient clipboard cache files from %TEMP%\FileUC.
pub fn cleanup_clipboard_temp_files() {
    let temp_dir = std::env::temp_dir().join("FileUC");
    if temp_dir.exists() {
        let _ = std::fs::remove_file(temp_dir.join("clipboard_image.png"));
        let _ = std::fs::remove_file(temp_dir.join("clipboard_link.txt"));
        let _ = std::fs::remove_file(temp_dir.join("clipboard_text.txt"));
    }
}

pub fn get_clipboard_text() -> Option<String> {
    unsafe {
        use windows::Win32::Foundation::HGLOBAL;
        use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
        use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

        if OpenClipboard(None).is_err() {
            return None;
        }
        let res = match GetClipboardData(13) {
            // CF_UNICODETEXT = 13
            Ok(handle) => {
                let hglobal = HGLOBAL(handle.0);
                let ptr = GlobalLock(hglobal);
                if !ptr.is_null() {
                    let total_bytes = GlobalSize(hglobal);
                    let max_u16 = (total_bytes / 2) as usize;
                    let u16_ptr = ptr as *const u16;
                    let mut len = 0;
                    while len < max_u16 && *u16_ptr.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(u16_ptr, len);
                    let text = String::from_utf16_lossy(slice);
                    let _ = GlobalUnlock(hglobal);
                    let cleaned = text.replace("\r\n", "\n").replace('\r', "");
                    Some(cleaned)
                } else {
                    None
                }
            }
            Err(_) => None,
        };
        let _ = CloseClipboard();
        res
    }
}

pub fn set_clipboard_text(text: &str) -> bool {
    unsafe {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
        use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

        if OpenClipboard(None).is_err() {
            return false;
        }
        let _ = EmptyClipboard();

        let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes_needed = utf16.len() * std::mem::size_of::<u16>();

        let success = match GlobalAlloc(GMEM_MOVEABLE, bytes_needed) {
            Ok(hglobal) if !hglobal.is_invalid() => {
                let ptr = GlobalLock(hglobal);
                if !ptr.is_null() {
                    std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr as *mut u16, utf16.len());
                    let _ = GlobalUnlock(hglobal);
                    // CF_UNICODETEXT = 13
                    SetClipboardData(13, HANDLE(hglobal.0)).is_ok()
                } else {
                    false
                }
            }
            _ => false,
        };

        let _ = CloseClipboard();
        success
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get_clipboard_text() {
        let test_str = "https://fileuc.share/test-link-12345";
        let ok = set_clipboard_text(test_str);
        if ok {
            let got = get_clipboard_text();
            assert_eq!(got.as_deref(), Some(test_str));
        }
    }
}

