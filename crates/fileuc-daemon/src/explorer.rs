use std::path::PathBuf;
use windows::core::{Interface, VARIANT};
use windows::Win32::System::Com::{CoCreateInstance, IDataObject, IServiceProvider, CLSCTX_ALL};
use windows::Win32::System::Ole::IOleWindow;
use windows::Win32::UI::Shell::{
    IShellBrowser, IShellView, IShellWindows, ShellWindows, SID_STopLevelBrowser, SVGIO_SELECTION,
    SWC_DESKTOP, SWFO_NEEDDISPATCH,
};
use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, GetForegroundWindow, GA_ROOT};
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

pub fn get_selected_files() -> Vec<PathBuf> {
    unsafe {
        let sw: IShellWindows = match CoCreateInstance(&ShellWindows, None, CLSCTX_ALL) {
            Ok(w) => w,
            Err(_) => return Vec::new(),
        };

        let fg_hwnd = GetForegroundWindow();
        let fg_root = if !fg_hwnd.is_invalid() {
            GetAncestor(fg_hwnd, GA_ROOT)
        } else {
            fg_hwnd
        };

        // 1. Check if Desktop is the foreground window first
        let empty = VARIANT::default();
        let mut desktop_hwnd_val = 0i32;
        let desktop_browser: Option<IShellBrowser> = if let Ok(disp) =
            sw.FindWindowSW(&empty, &empty, SWC_DESKTOP, &mut desktop_hwnd_val, SWFO_NEEDDISPATCH)
        {
            disp.cast::<IServiceProvider>()
                .ok()
                .and_then(|sp| sp.QueryService::<IShellBrowser>(&SID_STopLevelBrowser).ok())
        } else {
            None
        };

        let is_desktop_foreground = !fg_hwnd.is_invalid()
            && desktop_hwnd_val != 0
            && (fg_hwnd.0 as isize == desktop_hwnd_val as isize
                || fg_root.0 as isize == desktop_hwnd_val as isize);

        if is_desktop_foreground {
            if let Some(ref browser) = desktop_browser {
                if let Some(files) = extract_files_from_browser(browser) {
                    return files;
                }
            }
        }

        // 2. Enumerate open Explorer windows, prioritizing the active foreground window
        let count = sw.Count().unwrap_or(0);
        let mut foreground_browser = None;
        let mut other_browsers = Vec::new();

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

            let is_fg = match win_hwnd {
                Some(h) => h == fg_hwnd || h == fg_root,
                None => false,
            };

            if is_fg && foreground_browser.is_none() {
                foreground_browser = Some(browser);
            } else {
                other_browsers.push(browser);
            }
        }

        // Test the foreground Explorer window first
        if let Some(ref browser) = foreground_browser {
            if let Some(files) = extract_files_from_browser(browser) {
                return files;
            }
        }

        // Fall back to any open Explorer window with a selection
        for browser in &other_browsers {
            if let Some(files) = extract_files_from_browser(browser) {
                return files;
            }
        }

        // Fall back to Desktop if open windows had no selection
        if !is_desktop_foreground {
            if let Some(ref browser) = desktop_browser {
                if let Some(files) = extract_files_from_browser(browser) {
                    return files;
                }
            }
        }
    }

    Vec::new()
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
