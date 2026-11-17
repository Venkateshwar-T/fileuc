use std::path::PathBuf;
use windows::core::{Interface, VARIANT};
use windows::Win32::System::Com::{CoCreateInstance, IDataObject, IServiceProvider, CLSCTX_ALL};
use windows::Win32::System::Ole::IOleWindow;
use windows::Win32::UI::Shell::{
    IShellBrowser, IShellView, IShellWindows, ShellWindows, SID_STopLevelBrowser, SVGIO_SELECTION,
    SWC_DESKTOP, SWFO_NEEDDISPATCH,
};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetClassNameW, GetCursorPos, GetForegroundWindow, WindowFromPoint, GA_ROOT,
};
use fileuc_hud::drop_target::parse_cf_hdrop;

fn extract_files_from_browser(browser: &IShellBrowser) -> Option<Vec<PathBuf>> {
    unsafe {
        let view: IShellView = browser.QueryActiveShellView().ok()?;
        let data_obj: IDataObject = view.GetItemObject(SVGIO_SELECTION).ok()?;
        let files = parse_cf_hdrop(&data_obj).ok()?;
        if !files.is_empty() {
            Some(files)
        } else {
            None
        }
    }
}

pub fn is_explorer_or_desktop_hwnd(hwnd: HWND) -> bool {
    if hwnd.is_invalid() || hwnd.0.is_null() {
        return false;
    }
    let mut class_name = [0u16; 64];
    let len = unsafe { GetClassNameW(hwnd, &mut class_name) };
    if len > 0 {
        let name = String::from_utf16_lossy(&class_name[..len as usize]);
        matches!(name.as_str(), "CabinetWClass" | "ExploreWClass" | "Progman" | "WorkerW")
    } else {
        false
    }
}

pub fn is_explorer_window(hwnd: HWND) -> bool {
    if hwnd.is_invalid() || hwnd.0.is_null() {
        return false;
    }
    let mut class_name = [0u16; 64];
    let len = unsafe { GetClassNameW(hwnd, &mut class_name) };
    if len > 0 {
        let name = String::from_utf16_lossy(&class_name[..len as usize]);
        matches!(name.as_str(), "CabinetWClass" | "ExploreWClass")
    } else {
        false
    }
}

pub fn is_desktop_window(hwnd: HWND) -> bool {
    if hwnd.is_invalid() || hwnd.0.is_null() {
        return false;
    }
    let mut class_name = [0u16; 64];
    let len = unsafe { GetClassNameW(hwnd, &mut class_name) };
    if len > 0 {
        let name = String::from_utf16_lossy(&class_name[..len as usize]);
        matches!(name.as_str(), "Progman" | "WorkerW")
    } else {
        false
    }
}

/// Checks whether the user is interacting with an Explorer or Desktop window at the given cursor coordinates.
/// Rejects Taskbar (Shell_TrayWnd), browsers, and external apps in <1µs.
pub fn is_explorer_or_desktop_at(cursor_x: i32, cursor_y: i32) -> bool {
    unsafe {
        let pt = POINT { x: cursor_x, y: cursor_y };
        let hover_hwnd = WindowFromPoint(pt);
        if !hover_hwnd.is_invalid() && !hover_hwnd.0.is_null() {
            let hover_root = GetAncestor(hover_hwnd, GA_ROOT);
            if is_explorer_or_desktop_hwnd(hover_hwnd) || is_explorer_or_desktop_hwnd(hover_root) {
                return true;
            }
        }

        let fg_hwnd = GetForegroundWindow();
        if !fg_hwnd.is_invalid() && !fg_hwnd.0.is_null() {
            let fg_root = GetAncestor(fg_hwnd, GA_ROOT);
            if is_explorer_or_desktop_hwnd(fg_hwnd) || is_explorer_or_desktop_hwnd(fg_root) {
                return true;
            }
        }

        false
    }
}

/// Retrieves selected files specifically from the Explorer or Desktop window under the cursor
/// or in the foreground. Traverses all tabs in tabbed Windows 11 Explorer windows so active
/// tab selections are reliably returned. Never queries unrelated background windows.
pub fn get_selected_files_at(cursor_x: i32, cursor_y: i32) -> Vec<PathBuf> {
    unsafe {
        let pt = POINT { x: cursor_x, y: cursor_y };
        let hover_hwnd = WindowFromPoint(pt);
        let hover_root = if !hover_hwnd.is_invalid() && !hover_hwnd.0.is_null() {
            GetAncestor(hover_hwnd, GA_ROOT)
        } else {
            hover_hwnd
        };

        let fg_hwnd = GetForegroundWindow();
        let fg_root = if !fg_hwnd.is_invalid() && !fg_hwnd.0.is_null() {
            GetAncestor(fg_hwnd, GA_ROOT)
        } else {
            fg_hwnd
        };

        // If neither the window under cursor nor the foreground window is Explorer or Desktop, return empty immediately.
        let hover_is_exp = is_explorer_or_desktop_hwnd(hover_hwnd) || is_explorer_or_desktop_hwnd(hover_root);
        let fg_is_exp = is_explorer_or_desktop_hwnd(fg_hwnd) || is_explorer_or_desktop_hwnd(fg_root);
        if !hover_is_exp && !fg_is_exp {
            return Vec::new();
        }

        let sw: IShellWindows = match CoCreateInstance(&ShellWindows, None, CLSCTX_ALL) {
            Ok(w) => w,
            Err(_) => return Vec::new(),
        };

        // 1. Check Desktop if cursor or foreground is on Desktop
        let is_desktop_target = is_desktop_window(hover_hwnd)
            || is_desktop_window(hover_root)
            || is_desktop_window(fg_hwnd)
            || is_desktop_window(fg_root);

        if is_desktop_target {
            let empty = VARIANT::default();
            let mut desktop_hwnd_val = 0i32;
            if let Ok(disp) = sw.FindWindowSW(&empty, &empty, SWC_DESKTOP, &mut desktop_hwnd_val, SWFO_NEEDDISPATCH) {
                if let Ok(sp) = disp.cast::<IServiceProvider>() {
                    if let Ok(browser) = sp.QueryService::<IShellBrowser>(&SID_STopLevelBrowser) {
                        if let Some(files) = extract_files_from_browser(&browser) {
                            let valid: Vec<PathBuf> = files.into_iter().filter(|p| is_supported_user_file(p)).collect();
                            if !valid.is_empty() {
                                return valid;
                            }
                        }
                    }
                }
            }
        }

        // 2. Check Explorer tabs if target is an Explorer window
        let target_root = if is_explorer_window(hover_root) {
            hover_root
        } else if is_explorer_window(fg_root) {
            fg_root
        } else if is_explorer_window(hover_hwnd) {
            hover_hwnd
        } else if is_explorer_window(fg_hwnd) {
            fg_hwnd
        } else {
            HWND::default()
        };

        if !target_root.is_invalid() && !target_root.0.is_null() {
            let count = sw.Count().unwrap_or(0);
            for i in 0..count {
                let var = VARIANT::from(i);
                let disp = match sw.Item(&var) {
                    Ok(d) => d,
                    Err(_) => continue,
                };

                let sp: IServiceProvider = match disp.cast() {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                let browser: IShellBrowser = match sp.QueryService(&SID_STopLevelBrowser) {
                    Ok(b) => b,
                    Err(_) => continue,
                };

                let win_hwnd = browser
                    .cast::<IOleWindow>()
                    .ok()
                    .and_then(|ole| ole.GetWindow().ok());

                let belongs = match win_hwnd {
                    Some(h) => {
                        h == target_root
                            || GetAncestor(h, GA_ROOT) == target_root
                            || h == hover_hwnd
                            || h == fg_hwnd
                            || (h.0 as isize == target_root.0 as isize)
                    }
                    None => false,
                };

                if belongs {
                    if let Some(files) = extract_files_from_browser(&browser) {
                        let valid: Vec<PathBuf> = files.into_iter().filter(|p| is_supported_user_file(p)).collect();
                        if !valid.is_empty() {
                            return valid;
                        }
                    }
                }
            }
        }
    }

    Vec::new()
}

pub fn get_foreground_selected_files() -> Option<Vec<PathBuf>> {
    let files = get_selected_files();
    if !files.is_empty() {
        Some(files)
    } else {
        None
    }
}

pub fn get_selected_files() -> Vec<PathBuf> {
    unsafe {
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        get_selected_files_at(pt.x, pt.y)
    }
}

/// Determines if a file path is a standard user data file eligible for FileUC conversions.
/// Rejects:
/// - Windows shell virtual objects / GUID namespaces (e.g. ::{645FF040...} Recycle Bin, ::{20D04FE0...} This PC)
/// - Non-existent file paths
/// - Windows system files & folders ($Recycle.Bin, desktop.ini, thumbs.db, System Volume Information)
pub fn is_supported_user_file(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy();
    if path_str.contains("::{") || path_str.starts_with("::") || path_str.starts_with("shell:") {
        return false;
    }
    if !path.exists() {
        return false;
    }

    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n.to_lowercase(),
        None => return false,
    };

    if file_name.starts_with("$recycle.bin")
        || file_name == "desktop.ini"
        || file_name == "thumbs.db"
        || file_name == "system volume information"
        || file_name == "recycle bin"
    {
        return false;
    }

    true
}

pub fn select_file_in_explorer(path: &std::path::Path) {
    if !path.exists() {
        return;
    }
    let path_str = path.to_string_lossy().to_string();
    let mut cmd = std::process::Command::new("explorer");
    cmd.arg(format!("/select,{}", path_str));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd.spawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_is_supported_user_file_rejects_virtual_and_nonexistent() {
        // Virtual shell items
        assert!(!is_supported_user_file(Path::new("::{645FF040-5081-101B-9F08-00AA002F954E}")));
        assert!(!is_supported_user_file(Path::new("::{20D04FE0-3AEA-1069-A2D8-08002B30309D}")));
        assert!(!is_supported_user_file(Path::new("::virtual_folder")));

        // Non-existent path
        assert!(!is_supported_user_file(Path::new(r"C:\non_existent_folder_xyz\test.pdf")));
    }

    #[test]
    fn test_is_supported_user_file_allows_real_files_including_exe() {
        let temp = std::env::temp_dir();
        let exe_path = temp.join("test_app.exe");
        let pdf_path = temp.join("valid_doc.pdf");
        let recycle_virtual = Path::new("::{645FF040-5081-101B-9F08-00AA002F954E}");

        std::fs::write(&exe_path, b"MZ dummy exe").unwrap();
        std::fs::write(&pdf_path, b"%PDF-1.4").unwrap();

        assert!(is_supported_user_file(&exe_path), "Real .exe files on disk must be supported for zipping/sharing");
        assert!(is_supported_user_file(&pdf_path), "Real PDF files must be supported");
        assert!(!is_supported_user_file(recycle_virtual), "Virtual shell items must be rejected");

        let _ = std::fs::remove_file(exe_path);
        let _ = std::fs::remove_file(pdf_path);
    }
}
