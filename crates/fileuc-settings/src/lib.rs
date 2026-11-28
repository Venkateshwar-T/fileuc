pub mod autostart;
pub mod commands;
pub mod plugin_manager;
pub mod store;
pub mod updater;

use std::sync::atomic::{AtomicI32, Ordering};
use store::AppSettings;

pub static INITIAL_TAB: AtomicI32 = AtomicI32::new(0);

#[cfg(windows)]
pub fn activate_existing_window() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetForegroundWindow, ShowWindow, BringWindowToTop, SW_RESTORE, SW_SHOW,
    };
    use windows::core::w;

    unsafe {
        if let Ok(hwnd) = FindWindowW(None, w!("FileUC")) {
            if !hwnd.is_invalid() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = BringWindowToTop(hwnd);
                let _ = SetForegroundWindow(hwnd);
                return true;
            }
        }
    }
    false
}

#[cfg(not(windows))]
pub fn activate_existing_window() -> bool {
    false
}

#[cfg(windows)]
pub fn is_system_dark_mode() -> bool {
    unsafe {
        use windows::Win32::System::Registry::{
            RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
            REG_DWORD, REG_VALUE_TYPE,
        };
        use windows::core::w;

        let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        let val_name = w!("AppsUseLightTheme");
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, 0, KEY_READ, &mut hkey).is_ok() {
            let mut data: u32 = 0;
            let mut data_len = std::mem::size_of::<u32>() as u32;
            let mut val_type = REG_VALUE_TYPE::default();
            let res = RegQueryValueExW(
                hkey,
                val_name,
                None,
                Some(&mut val_type),
                Some(&mut data as *mut u32 as *mut u8),
                Some(&mut data_len),
            );
            let _ = RegCloseKey(hkey);
            if res.is_ok() && val_type == REG_DWORD {
                return data == 0; // 0 = dark mode, 1 = light mode
            }
        }
    }
    false
}

#[cfg(not(windows))]
pub fn is_system_dark_mode() -> bool {
    false
}

pub fn run_tauri(initial_tab: i32) {
    #[cfg(windows)]
    {
        if activate_existing_window() {
            return;
        }
    }

    INITIAL_TAB.store(initial_tab, Ordering::Relaxed);
    updater::cleanup_old_updates();

    tauri::Builder::default()
        .setup(|app| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let icon_bytes = include_bytes!("../icons/icon.png");
                if let Ok(icon) = tauri::image::Image::from_bytes(icon_bytes) {
                    let _ = window.set_icon(icon);
                }

                #[cfg(windows)]
                {
                    use windows::Win32::Foundation::{HWND, WPARAM, LPARAM};
                    use windows::Win32::UI::WindowsAndMessaging::{
                        SendMessageW, WM_SETICON, ICON_BIG, ICON_SMALL, LoadImageW, IMAGE_ICON, LR_SHARED,
                    };
                    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
                    use windows::core::PCWSTR;

                    if let Ok(hwnd_raw) = window.hwnd() {
                        let hwnd = HWND(hwnd_raw.0 as _);
                        unsafe {
                            if let Ok(hinst) = GetModuleHandleW(None) {
                                let icon_id = PCWSTR(1 as _);
                                if let Ok(hicon_big) = LoadImageW(hinst, icon_id, IMAGE_ICON, 32, 32, LR_SHARED) {
                                    let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_BIG as _), LPARAM(hicon_big.0 as _));
                                }
                                if let Ok(hicon_sm) = LoadImageW(hinst, icon_id, IMAGE_ICON, 16, 16, LR_SHARED) {
                                    let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_SMALL as _), LPARAM(hicon_sm.0 as _));
                                }
                            }
                        }
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::toggle_service,
            commands::get_plugin_status,
            commands::install_plugin,
            commands::uninstall_plugin,
            commands::get_initial_tab,
            commands::get_system_theme,
            commands::check_for_updates,
            commands::open_release_url,
            commands::install_update_now,
            commands::get_active_conversion,
            commands::cancel_active_conversion,
            commands::pick_receive_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(Clone, Default)]
pub struct SettingsController;

impl SettingsController {
    pub fn new<F, T>(_on_save: F, _on_toggle_service: T) -> Self
    where
        F: Fn(AppSettings) + Send + Sync + 'static,
        T: Fn(bool) + Send + Sync + 'static,
    {
        Self
    }

    pub fn spawn<F, T>(on_save: F, on_toggle_service: T) -> Self
    where
        F: Fn(AppSettings) + Send + Sync + 'static,
        T: Fn(bool) + Send + Sync + 'static,
    {
        Self::new(on_save, on_toggle_service)
    }

    pub fn show(&self, settings: AppSettings) {
        self.show_tab(settings, -1);
    }

    pub fn show_tab(&self, _settings: AppSettings, tab: i32) {
        #[cfg(windows)]
        {
            if activate_existing_window() {
                return;
            }
        }

        if let Ok(exe) = std::env::current_exe() {
            let mut cmd = std::process::Command::new(exe);
            cmd.arg("--settings");
            if tab >= 0 {
                cmd.arg("--tab").arg(tab.to_string());
            }
            let _ = cmd.spawn();
        }
    }

    pub fn set_service_enabled(&self, _enabled: bool) {}
    pub fn hide(&self) {}
    pub fn quit(&self) {}
}
