use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Shell::{
    FileOpenDialog, IFileOpenDialog, FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM,
    FOS_NOCHANGEDIR, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, BringWindowToTop, CreateWindowExW, DestroyWindow,
    GetForegroundWindow, GetSystemMetrics, GetWindowThreadProcessId, PostThreadMessageW,
    SetForegroundWindow, SetLayeredWindowAttributes, ASFW_ANY, LWA_ALPHA, SM_CXSCREEN, SM_CYSCREEN,
    WM_NULL, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
};

static PICKER_OPEN: AtomicBool = AtomicBool::new(false);

struct PickerGuard;

impl Drop for PickerGuard {
    fn drop(&mut self) {
        PICKER_OPEN.store(false, Ordering::SeqCst);
    }
}

struct OwnerWindowGuard(HWND);

impl Drop for OwnerWindowGuard {
    fn drop(&mut self) {
        if !self.0 .0.is_null() {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }
}

/// Spawns a non-blocking Windows multi-file picker dialog on a dedicated COM STA thread.
/// Returns `None` if a picker dialog is already open.
/// Wakes `notify_thread_id` with `WM_NULL` once the dialog closes.
pub fn spawn_multi_file_picker(notify_thread_id: u32) -> Option<Receiver<Option<Vec<PathBuf>>>> {
    if PICKER_OPEN.swap(true, Ordering::SeqCst) {
        return None;
    }

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _guard = PickerGuard;
        let picked = unsafe { run_multi_file_dialog() };
        let _ = tx.send(picked);
        if notify_thread_id != 0 {
            unsafe {
                let _ = PostThreadMessageW(notify_thread_id, WM_NULL, WPARAM(0), LPARAM(0));
            }
        }
    });

    Some(rx)
}

unsafe fn create_focused_owner() -> OwnerWindowGuard {
    let hinst = GetModuleHandleW(None).unwrap_or_default();
    let cx = GetSystemMetrics(SM_CXSCREEN) / 2;
    let cy = GetSystemMetrics(SM_CYSCREEN) / 2;
    let owner = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_LAYERED,
        w!("STATIC"),
        w!(""),
        WS_POPUP | WS_VISIBLE,
        cx,
        cy,
        0,
        0,
        None,
        None,
        hinst,
        None,
    )
    .unwrap_or(HWND(std::ptr::null_mut()));

    if !owner.0.is_null() {
        let _ = SetLayeredWindowAttributes(owner, COLORREF(0), 0, LWA_ALPHA);
        let _ = AllowSetForegroundWindow(ASFW_ANY);

        let fg = GetForegroundWindow();
        let fg_tid = if !fg.0.is_null() {
            GetWindowThreadProcessId(fg, None)
        } else {
            0
        };
        let my_tid = GetCurrentThreadId();
        let attached = fg_tid != 0 && fg_tid != my_tid && AttachThreadInput(my_tid, fg_tid, true).as_bool();

        let _ = SetForegroundWindow(owner);
        let _ = BringWindowToTop(owner);

        if attached {
            let _ = AttachThreadInput(my_tid, fg_tid, false);
        }
    }

    OwnerWindowGuard(owner)
}

unsafe fn run_multi_file_dialog() -> Option<Vec<PathBuf>> {
    let hr_init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    let did_init = hr_init.is_ok();

    let res = (|| -> Option<Vec<PathBuf>> {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL).ok()?;
        let mut opts = dialog.GetOptions().unwrap_or_default();
        opts |= FOS_ALLOWMULTISELECT
            | FOS_FORCEFILESYSTEM
            | FOS_NOCHANGEDIR
            | FOS_PATHMUSTEXIST
            | FOS_FILEMUSTEXIST;
        let _ = dialog.SetOptions(opts);
        let _ = dialog.SetTitle(w!("Select Files to Share with Phone"));
        let _ = dialog.SetOkButtonLabel(w!("Share"));

        let owner_guard = create_focused_owner();
        if dialog.Show(owner_guard.0).is_err() {
            return None;
        }
        drop(owner_guard);

        let results = dialog.GetResults().ok()?;
        let count = results.GetCount().ok()?;
        if count == 0 {
            return None;
        }

        let mut paths = Vec::with_capacity(count as usize);
        for idx in 0..count {
            if let Ok(item) = results.GetItemAt(idx) {
                if let Ok(pwstr) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                    let s = pwstr.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(pwstr.as_ptr() as *const _));
                    if !s.is_empty() {
                        let p = PathBuf::from(s);
                        if p.exists() {
                            paths.push(p);
                        }
                    }
                }
            }
        }

        if paths.is_empty() {
            None
        } else {
            Some(paths)
        }
    })();

    if did_init {
        CoUninitialize();
    }

    res
}
