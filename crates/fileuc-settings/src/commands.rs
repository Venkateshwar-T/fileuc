use crate::plugin_manager;
use crate::store::{AppSettings, SettingsStore};
use fileuc_types::{ActiveConversionInfo, IpcMessage};
use serde::Serialize;
use tauri::Emitter;
use windows::core::w;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Memory::{
    MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ,
};

pub fn notify_daemon(msg: IpcMessage) {
    if let Ok(payload) = serde_json::to_vec(&msg) {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .write(true)
            .open(r"\\.\pipe\fileuc_ipc_pipe")
        {
            use std::io::Write;
            let _ = file.write_all(&payload);
        }
    }
}

#[derive(Serialize)]
pub struct PluginStatus {
    pub ffmpeg_installed: bool,
    pub docx_installed: bool,
}

#[derive(Clone, Serialize)]
pub struct PluginProgressPayload {
    pub name: String,
    pub progress: f32,
    pub text: String,
    pub done: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub fn get_system_theme() -> String {
    if crate::is_system_dark_mode() {
        "Dark".to_string()
    } else {
        "Light".to_string()
    }
}

#[tauri::command]
pub fn get_initial_tab() -> i32 {
    crate::INITIAL_TAB.load(std::sync::atomic::Ordering::Relaxed)
}

#[tauri::command]
pub fn get_settings() -> Result<AppSettings, String> {
    Ok(SettingsStore::default().load())
}

#[tauri::command]
pub fn save_settings(new_settings: AppSettings) -> Result<(), String> {
    let store = SettingsStore::default();
    store.save(&new_settings).map_err(|e| e.to_string())?;
    let _ = crate::autostart::set_auto_start(new_settings.auto_start);
    notify_daemon(IpcMessage::ReloadSettings);
    Ok(())
}

#[tauri::command]
pub fn toggle_service(enabled: bool) -> Result<(), String> {
    let store = SettingsStore::default();
    let mut st = store.load();
    st.service_enabled = enabled;
    let _ = store.save(&st);
    notify_daemon(IpcMessage::ToggleService(enabled));
    Ok(())
}

#[tauri::command]
pub fn get_plugin_status() -> Result<PluginStatus, String> {
    Ok(PluginStatus {
        ffmpeg_installed: plugin_manager::is_ffmpeg_installed(),
        docx_installed: plugin_manager::is_docx_installed(),
    })
}

#[tauri::command]
pub fn install_plugin(app: tauri::AppHandle, name: String) -> Result<(), String> {
    let app_handle = app.clone();
    let plugin_name = name.clone();

    std::thread::spawn(move || {
        let n = plugin_name.clone();
        let app_cb = app_handle.clone();
        let progress_cb = move |progress: f32, text: &str| {
            let _ = app_cb.emit(
                "plugin-progress",
                PluginProgressPayload {
                    name: n.clone(),
                    progress,
                    text: text.to_string(),
                    done: false,
                    error: None,
                },
            );
        };

        let result = if plugin_name == "ffmpeg" {
            plugin_manager::install_ffmpeg(progress_cb)
        } else {
            plugin_manager::install_docx(progress_cb)
        };

        match result {
            Ok(()) => {
                let _ = app_handle.emit(
                    "plugin-progress",
                    PluginProgressPayload {
                        name: plugin_name,
                        progress: 1.0,
                        text: "Installation complete".to_string(),
                        done: true,
                        error: None,
                    },
                );
            }
            Err(e) => {
                let _ = app_handle.emit(
                    "plugin-progress",
                    PluginProgressPayload {
                        name: plugin_name,
                        progress: 0.0,
                        text: e.clone(),
                        done: true,
                        error: Some(e),
                    },
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn uninstall_plugin(name: String) -> Result<(), String> {
    if name == "ffmpeg" {
        plugin_manager::uninstall_ffmpeg().map_err(|e| e.to_string())?;
    } else {
        plugin_manager::uninstall_docx().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn check_for_updates(force: bool) -> Result<crate::updater::UpdateCheckResponse, String> {
    Ok(crate::updater::check_for_updates_internal(force))
}

#[tauri::command]
pub fn open_release_url(url: String) -> Result<(), String> {
    crate::updater::open_url_in_browser(&url)
}

#[tauri::command]
pub fn install_update_now(app_handle: tauri::AppHandle, download_url: String) -> Result<(), String> {
    crate::updater::download_and_install_update(app_handle, download_url)
}

#[tauri::command]
pub fn get_active_conversion() -> Option<ActiveConversionInfo> {
    unsafe {
        let handle = match OpenFileMappingW(FILE_MAP_READ.0, false, w!("Local\\FileUC_Conversion_State")) {
            Ok(h) => h,
            Err(_) => return None,
        };
        let view = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 1024);
        if view.Value.is_null() {
            let _ = CloseHandle(handle);
            return None;
        }

        let byte_ptr = view.Value as *const u8;
        let magic = *(byte_ptr as *const u32);
        if magic != 0x46494C45 {
            let _ = UnmapViewOfFile(view);
            let _ = CloseHandle(handle);
            return None;
        }

        std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
        let is_active = *byte_ptr.add(4) == 1;
        if !is_active {
            let _ = UnmapViewOfFile(view);
            let _ = CloseHandle(handle);
            return None;
        }

        let progress_permille = *(byte_ptr.add(8) as *const u32);
        let progress = (progress_permille as f32 / 1000.0).clamp(0.0, 1.0);

        let action_len = (*(byte_ptr.add(12) as *const u32) as usize).min(32);
        let action_slice = std::slice::from_raw_parts(byte_ptr.add(16), action_len);
        let action_display = std::str::from_utf8(action_slice).unwrap_or("").to_string();

        let eta_len = (*(byte_ptr.add(48) as *const u32) as usize).min(32);
        let eta_slice = std::slice::from_raw_parts(byte_ptr.add(52), eta_len);
        let eta_text = std::str::from_utf8(eta_slice).unwrap_or("").to_string();

        let summary_len = (*(byte_ptr.add(84) as *const u32) as usize).min(424);
        let summary_slice = std::slice::from_raw_parts(byte_ptr.add(88), summary_len);
        let file_summary = std::str::from_utf8(summary_slice).unwrap_or("").to_string();

        let _ = UnmapViewOfFile(view);
        let _ = CloseHandle(handle);

        Some(ActiveConversionInfo {
            is_active: true,
            file_summary,
            action_display,
            progress,
            eta_text,
        })
    }
}

#[tauri::command]
pub fn cancel_active_conversion() -> Result<(), String> {
    notify_daemon(IpcMessage::CancelActiveConversion);
    Ok(())
}

#[tauri::command]
pub fn pick_receive_folder() -> Result<Option<String>, String> {
    std::thread::spawn(|| -> Result<Option<String>, String> {
        unsafe {
            use windows::Win32::System::Com::{
                CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
                COINIT_APARTMENTTHREADED,
            };
            use windows::Win32::UI::Shell::{
                FileOpenDialog, IFileOpenDialog, FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR,
                FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
            };

            let hr_init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let did_init = hr_init.is_ok();

            let result = (|| -> Result<Option<String>, String> {
                let dialog: IFileOpenDialog =
                    CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)
                        .map_err(|e| format!("Failed to open folder dialog: {e}"))?;

                let mut opts = dialog.GetOptions().unwrap_or_default();
                opts |= FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_NOCHANGEDIR;
                let _ = dialog.SetOptions(opts);
                let _ = dialog.SetTitle(w!("Select Default Receive Folder"));

                match dialog.Show(None) {
                    Ok(()) => {
                        let item = dialog
                            .GetResult()
                            .map_err(|e| format!("Failed to read selected folder: {e}"))?;
                        let pwstr = item
                            .GetDisplayName(SIGDN_FILESYSPATH)
                            .map_err(|e| format!("Failed to read folder path: {e}"))?;
                        let path_str = pwstr.to_string().unwrap_or_default();
                        CoTaskMemFree(Some(pwstr.as_ptr() as *const _));
                        if path_str.is_empty() {
                            Ok(None)
                        } else {
                            Ok(Some(path_str))
                        }
                    }
                    Err(_) => Ok(None),
                }
            })();

            if did_init {
                CoUninitialize();
            }

            result
        }
    })
    .join()
    .map_err(|_| "Folder picker thread panicked".to_string())?
}
