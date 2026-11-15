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

