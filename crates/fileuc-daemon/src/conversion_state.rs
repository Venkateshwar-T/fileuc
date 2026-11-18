use std::ffi::c_void;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_ALL_ACCESS,
    MEMORY_MAPPED_VIEW_ADDRESS, PAGE_READWRITE,
};

const SHARED_MEM_NAME: PCWSTR = w!("Local\\FileUC_Conversion_State");
const SHARED_MEM_SIZE: usize = 1024;
const MAGIC: u32 = 0x46494C45; // "FILE"

pub struct SharedConversionWriter {
    mapping: HANDLE,
    ptr: *mut u8,
}

unsafe impl Send for SharedConversionWriter {}
unsafe impl Sync for SharedConversionWriter {}

impl SharedConversionWriter {
    pub fn new() -> Self {
        unsafe {
            let mapping = match CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                None,
                PAGE_READWRITE,
                0,
                SHARED_MEM_SIZE as u32,
                SHARED_MEM_NAME,
            ) {
                Ok(h) => h,
                Err(_) => {
                    return Self {
                        mapping: HANDLE(std::ptr::null_mut()),
                        ptr: std::ptr::null_mut(),
                    }
                }
            };

            let view = MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, SHARED_MEM_SIZE);

            let ptr = view.Value as *mut u8;
            if !ptr.is_null() {
                // Initialize buffer
                *(ptr as *mut u32) = MAGIC;
                *ptr.add(4) = 0; // is_active = false
            }

            Self { mapping, ptr }
        }
    }

    pub fn update_active(&self, summary: &str, action: &str, progress: f32, eta: &str) {
        if self.ptr.is_null() {
            return;
        }
        unsafe {
            let permille = (progress.clamp(0.0, 1.0) * 1000.0) as u32;
            *(self.ptr as *mut u32) = MAGIC;

            // Action string (max 32 bytes)
            let action_bytes = action.as_bytes();
            let action_len = action_bytes.len().min(32);
            *(self.ptr.add(12) as *mut u32) = action_len as u32;
            std::ptr::copy_nonoverlapping(action_bytes.as_ptr(), self.ptr.add(16), action_len);

            // ETA string (max 32 bytes)
            let eta_bytes = eta.as_bytes();
            let eta_len = eta_bytes.len().min(32);
            *(self.ptr.add(48) as *mut u32) = eta_len as u32;
            std::ptr::copy_nonoverlapping(eta_bytes.as_ptr(), self.ptr.add(52), eta_len);

            // Summary string (max 424 bytes)
            let summary_bytes = summary.as_bytes();
            let summary_len = summary_bytes.len().min(424);
            *(self.ptr.add(84) as *mut u32) = summary_len as u32;
            std::ptr::copy_nonoverlapping(summary_bytes.as_ptr(), self.ptr.add(88), summary_len);

            // Progress
            *(self.ptr.add(8) as *mut u32) = permille;

            // Set active flag last so readers see initialized payload
            std::sync::atomic::fence(std::sync::atomic::Ordering::Release);
            *self.ptr.add(4) = 1;
        }
    }

    pub fn set_inactive(&self) {
        if self.ptr.is_null() {
            return;
        }
        unsafe {
            *self.ptr.add(4) = 0;
            std::sync::atomic::fence(std::sync::atomic::Ordering::Release);
        }
    }
}

impl Default for SharedConversionWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SharedConversionWriter {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.ptr as *mut c_void,
                });
            }
            if !self.mapping.is_invalid() && !self.mapping.0.is_null() {
                let _ = CloseHandle(self.mapping);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_conversion_writer_lifecycle() {
        let writer = SharedConversionWriter::new();
        assert!(!writer.ptr.is_null());

        writer.update_active("test_file.png", "PNG", 0.45, "~8 sec");
        unsafe {
            let magic = *(writer.ptr as *const u32);
            assert_eq!(magic, MAGIC);
            let is_active = *writer.ptr.add(4);
            assert_eq!(is_active, 1);
            let permille = *(writer.ptr.add(8) as *const u32);
            assert_eq!(permille, 450);

            let action_len = *(writer.ptr.add(12) as *const u32) as usize;
            let action_slice = std::slice::from_raw_parts(writer.ptr.add(16), action_len);
            assert_eq!(std::str::from_utf8(action_slice).unwrap(), "PNG");

            let eta_len = *(writer.ptr.add(48) as *const u32) as usize;
            let eta_slice = std::slice::from_raw_parts(writer.ptr.add(52), eta_len);
            assert_eq!(std::str::from_utf8(eta_slice).unwrap(), "~8 sec");

            let summary_len = *(writer.ptr.add(84) as *const u32) as usize;
            let summary_slice = std::slice::from_raw_parts(writer.ptr.add(88), summary_len);
            assert_eq!(std::str::from_utf8(summary_slice).unwrap(), "test_file.png");
        }

        writer.set_inactive();
        unsafe {
            let is_active = *writer.ptr.add(4);
            assert_eq!(is_active, 0);
        }
    }
}
