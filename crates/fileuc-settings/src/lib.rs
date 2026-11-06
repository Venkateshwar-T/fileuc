pub mod store;

slint::include_modules!();

use std::sync::{Arc, Mutex};
use store::{AppSettings, HudStyleChoice, ThemeChoice};

pub fn open_settings_window(settings: AppSettings) -> Option<AppSettings> {
    let window = SettingsWindow::new().ok()?;

    window.set_trigger_modifier(settings.trigger_modifier.clone().into());
    window.set_hud_style(
        match settings.hud_style {
            HudStyleChoice::Donut => "Donut",
            HudStyleChoice::Pill => "Pill",
        }
        .into(),
    );
    window.set_theme_choice(
        match settings.theme {
            ThemeChoice::System => "System",
            ThemeChoice::Dark => "Dark",
            ThemeChoice::Light => "Light",
        }
        .into(),
    );
    window.set_auto_start(settings.auto_start);

    let saved_result = Arc::new(Mutex::new(None));

    let window_weak = window.as_weak();
    let res_clone = saved_result.clone();
    let mut initial_settings = settings.clone();

    window.on_save_clicked(move || {
        if let Some(w) = window_weak.upgrade() {
            initial_settings.trigger_modifier = w.get_trigger_modifier().to_string();
            initial_settings.hud_style = if w.get_hud_style() == "Pill" {
                HudStyleChoice::Pill
            } else {
                HudStyleChoice::Donut
            };
            initial_settings.theme = match w.get_theme_choice().as_str() {
                "Dark" => ThemeChoice::Dark,
                "Light" => ThemeChoice::Light,
                _ => ThemeChoice::System,
            };
            initial_settings.auto_start = w.get_auto_start();

            *res_clone.lock().unwrap() = Some(initial_settings.clone());
            let _ = w.hide();
        }
    });

    let window_weak2 = window.as_weak();
    window.on_cancel_clicked(move || {
        if let Some(w) = window_weak2.upgrade() {
            let _ = w.hide();
        }
    });

    let _ = window.show();
    let _ = slint::run_event_loop();

    let res = saved_result.lock().unwrap().take();
    res
}
