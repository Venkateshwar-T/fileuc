use windows::Win32::System::Registry::{RegDeleteTreeW, HKEY_CURRENT_USER};
use windows::core::w;

pub fn cleanup_context_menu() {
    unsafe {
        let _ = RegDeleteTreeW(
            HKEY_CURRENT_USER,
            w!("Software\\Classes\\*\\shell\\FileUCPreview"),
        );
    }
}
