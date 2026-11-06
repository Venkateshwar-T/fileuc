use fileuc_settings::store::{OutputMode, SettingsStore, ThemeChoice};

#[test]
fn defaults_when_no_file() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(tmp.path().join("settings.json"));
    let s = store.load();
    assert_eq!(s.trigger_modifier, "Ctrl");
    assert!(matches!(s.output_mode, OutputMode::NextToOriginal));
    assert!(matches!(s.theme, ThemeChoice::System));
    assert!(!s.auto_start);
}

#[test]
fn roundtrip_persistence() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    let store = SettingsStore::new(path.clone());
    let mut s = store.load();
    s.trigger_modifier = "Alt".into();
    s.auto_start = true;
    store.save(&s).unwrap();

    let store2 = SettingsStore::new(path);
    let loaded = store2.load();
    assert_eq!(loaded.trigger_modifier, "Alt");
    assert!(loaded.auto_start);
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
