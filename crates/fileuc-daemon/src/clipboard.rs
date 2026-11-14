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
