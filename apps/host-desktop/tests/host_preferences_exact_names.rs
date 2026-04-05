use host_desktop::preferences::{HostPreferences, HostPreferencesStore};

#[test]
fn host_preferences_roundtrip_json() {
    let prefs = HostPreferences {
        selected_device_id: Some("device-1".into()),
        selected_source_id: Some("window-helper-1".into()),
    };

    let json = serde_json::to_string(&prefs).unwrap();
    let decoded: HostPreferences = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, prefs);
}

#[test]
fn load_missing_preferences_file_returns_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing-preferences.json");
    let store = HostPreferencesStore::new(path);

    let prefs = store.load().unwrap();
    assert_eq!(prefs, HostPreferences::default());
}
