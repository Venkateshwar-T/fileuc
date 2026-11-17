use crate::plugin_manager;
use crate::store::{AppSettings, SettingsStore};
use fileuc_types::IpcMessage;
use serde::Serialize;
use tauri::Emitter;

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
