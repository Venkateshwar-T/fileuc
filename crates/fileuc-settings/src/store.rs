use crate::updater::PendingUpdate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputMode {
    NextToOriginal,
    CustomFolder(PathBuf),
}

impl Default for OutputMode {
    fn default() -> Self {
        OutputMode::NextToOriginal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeChoice {
    System,
    Dark,
    Light,
}

impl Default for ThemeChoice {
    fn default() -> Self {
        ThemeChoice::System
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HudStyleChoice {
    Donut,
    Pill,
}

impl Default for HudStyleChoice {
    fn default() -> Self {
        HudStyleChoice::Donut
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_trigger_modifier")]
    pub trigger_modifier: String,
    #[serde(default = "default_qr_shortcut")]
    pub qr_shortcut: String,
    #[serde(default = "default_search_shortcut")]
    pub search_shortcut: String,
    #[serde(default)]
    pub receive_shortcut: String,
    #[serde(default)]
    pub receive_folder: Option<PathBuf>,
    #[serde(default)]
    pub hud_style: HudStyleChoice,
    #[serde(default)]
    pub output_mode: OutputMode,
    #[serde(default)]
    pub theme: ThemeChoice,
    #[serde(default = "default_accent_color")]
    pub accent_color: String,
    #[serde(default = "default_true")]
    pub auto_start: bool,
    #[serde(default = "default_true")]
    pub show_tray_icon: bool,
    #[serde(default = "default_true")]
    pub service_enabled: bool,
    #[serde(default)]
    pub ffmpeg_path: Option<PathBuf>,
    #[serde(default)]
    pub pending_update: Option<PendingUpdate>,
    #[serde(default)]
    pub detailed_progress: bool,
}

fn default_trigger_modifier() -> String {
    "Ctrl".to_string()
}

fn default_qr_shortcut() -> String {
    "Ctrl+Shift+Q".to_string()
}

fn default_search_shortcut() -> String {
    "Ctrl+Space".to_string()
}

fn default_accent_color() -> String {
    "#7C5CFC".to_string()
}

pub fn default_receive_folder() -> PathBuf {
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let p = PathBuf::from(profile);
        if !p.as_os_str().is_empty() {
            return p.join("Documents").join("FileUC Received");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home);
        if !p.as_os_str().is_empty() {
            return p.join("Documents").join("FileUC Received");
        }
    }
    PathBuf::from("FileUC Received")
}

impl AppSettings {
    pub fn effective_receive_folder(&self) -> PathBuf {
        self.receive_folder
            .as_ref()
            .filter(|p| !p.as_os_str().is_empty())
            .cloned()
            .unwrap_or_else(default_receive_folder)
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            trigger_modifier: default_trigger_modifier(),
            qr_shortcut: default_qr_shortcut(),
            search_shortcut: default_search_shortcut(),
            receive_shortcut: String::new(),
            receive_folder: None,
            hud_style: HudStyleChoice::default(),
            output_mode: OutputMode::default(),
            theme: ThemeChoice::default(),
            accent_color: default_accent_color(),
            auto_start: true,
            show_tray_icon: true,
            service_enabled: true,
            ffmpeg_path: None,
            pending_update: None,
            detailed_progress: false,
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new(Self::default_path())
    }
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> PathBuf {
        let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        PathBuf::from(app_data).join("FileUC").join("settings.json")
    }

    pub fn load(&self) -> AppSettings {
        if !self.path.exists() {
            return AppSettings::default();
        }
        let data = match std::fs::read_to_string(&self.path) {
            Ok(d) => d,
            Err(_) => return AppSettings::default(),
        };
        let mut settings: AppSettings = serde_json::from_str(&data).unwrap_or_default();
        if let Some(ref pending) = settings.pending_update {
            if !crate::updater::is_newer_version(&pending.version, env!("CARGO_PKG_VERSION")) {
                settings.pending_update = None;
                let _ = self.save(&settings);
            }
        }
        settings
    }

    pub fn save(&self, settings: &AppSettings) -> Result<(), std::io::Error> {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Atomic write: write to temporary file first, then atomically rename
        let tmp_path = self.path.with_extension(format!("tmp.{}", std::process::id()));
        std::fs::write(&tmp_path, json.as_bytes())?;

        if std::fs::rename(&tmp_path, &self.path).is_err() {
            let _ = std::fs::remove_file(&self.path);
            std::fs::rename(&tmp_path, &self.path)?;
        }
        Ok(())
    }
}
