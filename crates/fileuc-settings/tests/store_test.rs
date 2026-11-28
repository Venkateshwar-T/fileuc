use fileuc_settings::store::{OutputMode, SettingsStore, ThemeChoice};

#[test]
fn defaults_when_no_file() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(tmp.path().join("settings.json"));
    let s = store.load();
    assert_eq!(s.trigger_modifier, "Ctrl");
    assert!(matches!(s.output_mode, OutputMode::NextToOriginal));
    assert!(matches!(s.theme, ThemeChoice::System));
    assert!(s.auto_start);
    assert!(s.show_tray_icon);
    assert!(s.service_enabled);
}

#[test]
fn roundtrip_persistence() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    let store = SettingsStore::new(path.clone());
    let mut s = store.load();
    s.trigger_modifier = "Alt".into();
    s.auto_start = false;
    s.show_tray_icon = false;
    s.service_enabled = false;
    store.save(&s).unwrap();

    let store2 = SettingsStore::new(path);
    let loaded = store2.load();
    assert_eq!(loaded.trigger_modifier, "Alt");
    assert!(!loaded.auto_start);
    assert!(!loaded.show_tray_icon);
    assert!(!loaded.service_enabled);
}

#[test]
fn corrupt_file_returns_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    std::fs::write(&path, b"NOT JSON").unwrap();
    let store = SettingsStore::new(path);
    let s = store.load();
    assert_eq!(s.trigger_modifier, "Ctrl"); // defaults
}

#[test]
fn pending_update_persists_when_newer() {
    use fileuc_settings::updater::{PendingUpdate, UpdatePriority};
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    let store = SettingsStore::new(path.clone());
    let mut s = store.load();
    s.pending_update = Some(PendingUpdate {
        version: "99.0.0".to_string(),
        release_url: "https://github.com/Venkateshwar-T/fileuc/releases/tag/v99.0.0".to_string(),
        release_name: Some("FileUC v99.0.0".to_string()),
        download_url: Some("https://github.com/Venkateshwar-T/fileuc/releases/download/v99.0.0/FileUC-Setup.exe".to_string()),
        priority: UpdatePriority::Blocking,
        release_notes: Some("Critical fix.".to_string()),
    });
    store.save(&s).unwrap();

    let store2 = SettingsStore::new(path);
    let loaded = store2.load();
    let pending = loaded.pending_update.expect("pending_update should persist");
    assert_eq!(pending.version, "99.0.0");
    assert_eq!(pending.priority, UpdatePriority::Blocking);
}

#[test]
fn pending_update_cleared_when_current_or_older() {
    use fileuc_settings::updater::{PendingUpdate, UpdatePriority};
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    let store = SettingsStore::new(path.clone());
    let mut s = store.load();
    // 0.0.1 is older than current version
    s.pending_update = Some(PendingUpdate {
        version: "0.0.1".to_string(),
        release_url: "https://github.com/Venkateshwar-T/fileuc/releases/tag/v0.0.1".to_string(),
        release_name: Some("FileUC v0.0.1".to_string()),
        download_url: None,
        priority: UpdatePriority::Standard,
        release_notes: None,
    });
    store.save(&s).unwrap();

    let store2 = SettingsStore::new(path.clone());
    let loaded = store2.load();
    // Must be automatically cleared on load
    assert!(loaded.pending_update.is_none());

    // File should also have been updated to None
    let store3 = SettingsStore::new(path);
    let loaded3 = store3.load();
    assert!(loaded3.pending_update.is_none());
}

#[test]
fn receive_settings_defaults_and_roundtrip() {
    use std::path::PathBuf;
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    let store = SettingsStore::new(path.clone());

    let mut s = store.load();
    assert_eq!(s.receive_shortcut, "");
    assert!(s.receive_folder.is_none());
    let default_dir = s.effective_receive_folder();
    assert!(default_dir.ends_with(PathBuf::from("Documents").join("FileUC Received")));

    let custom = tmp.path().join("CustomInbox");
    s.receive_shortcut = "Ctrl+Shift+R".to_string();
    s.receive_folder = Some(custom.clone());
    store.save(&s).unwrap();

    let reloaded = SettingsStore::new(path).load();
    assert_eq!(reloaded.receive_shortcut, "Ctrl+Shift+R");
    assert_eq!(reloaded.receive_folder.as_ref(), Some(&custom));
    assert_eq!(reloaded.effective_receive_folder(), custom);
}
