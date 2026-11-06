use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;

#[derive(Debug)]
pub enum InstanceError {
    AlreadyRunning,
    WindowsError(windows::core::Error),
}

pub struct SingleInstanceGuard {
    handle: HANDLE,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

unsafe impl Send for SingleInstanceGuard {}
unsafe impl Sync for SingleInstanceGuard {}

pub fn acquire(name: &str) -> Result<SingleInstanceGuard, InstanceError> {
    let wide_name = HSTRING::from(name);
    unsafe {
        let handle = CreateMutexW(None, true, PCWSTR::from_raw(wide_name.as_ptr()))
            .map_err(InstanceError::WindowsError)?;

        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return Err(InstanceError::AlreadyRunning);
        }

        Ok(SingleInstanceGuard { handle })
    }
}
