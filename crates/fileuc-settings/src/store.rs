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
}

fn default_trigger_modifier() -> String {
    "Ctrl".to_string()
}

fn default_qr_shortcut() -> String {
    "Ctrl+Shift+Q".to_string()
}

fn default_accent_color() -> String {
    "#7C5CFC".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            trigger_modifier: default_trigger_modifier(),
            qr_shortcut: default_qr_shortcut(),
            hud_style: HudStyleChoice::default(),
            output_mode: OutputMode::default(),
            theme: ThemeChoice::default(),
            accent_color: default_accent_color(),
            auto_start: true,
            show_tray_icon: true,
            service_enabled: true,
            ffmpeg_path: None,
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
        serde_json::from_str(&data).unwrap_or_default()
    }

    pub fn save(&self, settings: &AppSettings) -> Result<(), std::io::Error> {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&self.path, json)
    }
}
