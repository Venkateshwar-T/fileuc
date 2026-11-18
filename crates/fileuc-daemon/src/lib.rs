pub mod autostart;
pub mod clipboard;
pub mod cloudflared;
pub mod context_menu;
pub mod conversion_state;
pub mod dispatcher;
pub mod explorer;
pub mod ipc;
pub mod order_tracker;
pub mod pinggy;
pub mod single_instance;
pub mod share_server;
pub mod tray;
pub mod wifi;
pub mod qr_export;

pub use qr_export::save_qr_to_desktop;

pub fn trim_working_set() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}
