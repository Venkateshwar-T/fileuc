pub mod autostart;
pub mod dispatcher;
pub mod explorer;
pub mod ipc;
pub mod single_instance;
pub mod share_server;
pub mod tray;

pub fn trim_working_set() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}
