use std::path::PathBuf;
use std::sync::mpsc::Sender;
use windows::core::implement;
use windows::Win32::Foundation::{BOOL, HWND, POINT, POINTL};
use windows::Win32::System::Com::{IDataObject, FORMATETC, TYMED_HGLOBAL};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::{
    ReleaseStgMedium, CF_HDROP, DROPEFFECT_COPY, DROPEFFECT_NONE, DROPEFFECT, IDropTarget,
    IDropTarget_Impl,
};

const DVASPECT_CONTENT: u32 = 1;

#[derive(Debug)]
pub enum DropError {
    InvalidData,
    GlobalLockFailed,
    OleError(windows::core::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropTargetEvent {
    FilesEntered(Vec<PathBuf>),
    DragOver { x: i32, y: i32 },
    DragLeft,
    Dropped { x: i32, y: i32 },
    NoFiles,
}

#[repr(C)]
struct DropFilesHeader {
    p_files: u32,
    pt: POINT,
    f_nc: BOOL,
    f_wide: BOOL,
}

pub fn create_mock_hdrop(paths: &[PathBuf]) -> Vec<u8> {
    let header_size = std::mem::size_of::<DropFilesHeader>() as u32;
    let mut data = Vec::new();

    let header = DropFilesHeader {
        p_files: header_size,
        pt: POINT { x: 0, y: 0 },
        f_nc: BOOL(0),
        f_wide: BOOL(1), // UTF-16
    };

    let header_bytes = unsafe {
        std::slice::from_raw_parts(
            &header as *const _ as *const u8,
            header_size as usize,
        )
    };
    data.extend_from_slice(header_bytes);

    if !paths.is_empty() {
        for path in paths {
            let s = path.to_string_lossy();
            for u in s.encode_utf16() {
                data.extend_from_slice(&u.to_le_bytes());
            }
            data.extend_from_slice(&0u16.to_le_bytes());
        }
        data.extend_from_slice(&0u16.to_le_bytes()); // Final double null
    }

    data
}

pub fn parse_hdrop_buffer(buf: &[u8]) -> Result<Vec<PathBuf>, DropError> {
    let header_size = std::mem::size_of::<DropFilesHeader>();
    if buf.len() < header_size {
        return Err(DropError::InvalidData);
    }

    let header = unsafe { &*(buf.as_ptr() as *const DropFilesHeader) };
    let offset = header.p_files as usize;
    if offset >= buf.len() {
        return Ok(Vec::new());
    }

    let is_wide = header.f_wide.as_bool();
    let mut paths = Vec::new();

    if is_wide {
        let u16_slice = unsafe {
            std::slice::from_raw_parts(
                buf[offset..].as_ptr() as *const u16,
                (buf.len() - offset) / 2,
            )
        };

        let mut current_chars = Vec::new();
        for &ch in u16_slice {
            if ch == 0 {
                if current_chars.is_empty() {
                    break; // double null
                }
                let s = String::from_utf16_lossy(&current_chars);
                paths.push(PathBuf::from(s));
                current_chars.clear();
            } else {
                current_chars.push(ch);
            }
        }
    } else {
        let mut current_bytes = Vec::new();
        for &b in &buf[offset..] {
            if b == 0 {
                if current_bytes.is_empty() {
                    break;
                }
                let s = String::from_utf8_lossy(&current_bytes).to_string();
                paths.push(PathBuf::from(s));
                current_bytes.clear();
            } else {
                current_bytes.push(b);
            }
        }
    }

    Ok(paths)
}

pub fn parse_cf_hdrop(data_object: &IDataObject) -> Result<Vec<PathBuf>, DropError> {
    let fmt = FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };

    unsafe {
        let mut medium = data_object.GetData(&fmt).map_err(DropError::OleError)?;

        let hglobal = medium.u.hGlobal;
        if hglobal.is_invalid() {
            ReleaseStgMedium(&mut medium);
            return Err(DropError::InvalidData);
        }

        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            ReleaseStgMedium(&mut medium);
            return Err(DropError::GlobalLockFailed);
        }

        let slice = std::slice::from_raw_parts(ptr as *const u8, 1024 * 1024);
        let result = parse_hdrop_buffer(slice);

        let _ = GlobalUnlock(hglobal);
        ReleaseStgMedium(&mut medium);
        result
    }
}

#[implement(IDropTarget)]
pub struct FileDropTarget {
    hwnd: HWND,
    sender: Sender<DropTargetEvent>,
    has_files: std::sync::atomic::AtomicBool,
}

impl FileDropTarget {
    pub fn new(hwnd: HWND, sender: Sender<DropTargetEvent>) -> Self {
        Self {
            hwnd,
            sender,
            has_files: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl IDropTarget_Impl for FileDropTarget_Impl {
    fn DragEnter(
        &self,
        pdataobj: Option<&IDataObject>,
        _grfkeystate: windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let mut files = Vec::new();
        if let Some(obj) = pdataobj {
            if let Ok(paths) = parse_cf_hdrop(obj) {
                files = paths;
            }
        }

        if !files.is_empty() {
            self.has_files.store(true, std::sync::atomic::Ordering::SeqCst);
            let _ = self.sender.send(DropTargetEvent::FilesEntered(files));
            unsafe {
                *pdweffect = DROPEFFECT_COPY;
            }
        } else {
            self.has_files.store(false, std::sync::atomic::Ordering::SeqCst);
            let _ = self.sender.send(DropTargetEvent::NoFiles);
            unsafe {
                *pdweffect = DROPEFFECT_NONE;
            }
        }

        Ok(())
    }

    fn DragOver(
        &self,
        _grfkeystate: windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        if self.has_files.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = self.sender.send(DropTargetEvent::DragOver { x: pt.x, y: pt.y });
            unsafe {
                *pdweffect = DROPEFFECT_COPY;
            }
        } else {
            unsafe {
                *pdweffect = DROPEFFECT_NONE;
            }
        }
        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        self.has_files.store(false, std::sync::atomic::Ordering::SeqCst);
        let _ = self.sender.send(DropTargetEvent::DragLeft);
        Ok(())
    }

    fn Drop(
        &self,
        _pdataobj: Option<&IDataObject>,
        _grfkeystate: windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        if self.has_files.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = self.sender.send(DropTargetEvent::Dropped { x: pt.x, y: pt.y });
            unsafe {
                *pdweffect = DROPEFFECT_COPY;
            }
        } else {
            unsafe {
                *pdweffect = DROPEFFECT_NONE;
            }
        }
        self.has_files.store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}
