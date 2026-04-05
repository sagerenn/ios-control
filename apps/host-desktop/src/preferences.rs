use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPreferences {
    pub selected_device_id: Option<String>,
    pub selected_source_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HostPreferencesStore {
    path: PathBuf,
}

impl HostPreferencesStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<HostPreferences> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => Ok(serde_json::from_str(&text).unwrap_or_default()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Ok(HostPreferences::default())
            }
            Err(_) => Ok(HostPreferences::default()),
        }
    }

    pub fn save(&self, prefs: &HostPreferences) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(prefs)?;
        std::fs::write(&self.path, json)?;
        Ok(())
    }

    pub fn default_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            return std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .map(|base| base.join("ios-control").join("host-preferences.json"));
        }

        #[cfg(not(target_os = "windows"))]
        {
            let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
            let home = std::env::var_os("HOME").map(PathBuf::from);
            Self::path_from_env(xdg.as_ref(), home.as_ref(), None)
        }
    }

    pub fn path_from_env(
        xdg_config_home: Option<&PathBuf>,
        home: Option<&PathBuf>,
        appdata: Option<&PathBuf>,
    ) -> Option<PathBuf> {
        if let Some(appdata) = appdata {
            return Some(appdata.join("ios-control").join("host-preferences.json"));
        }
        if let Some(xdg) = xdg_config_home {
            return Some(xdg.join("ios-control").join("host-preferences.json"));
        }
        home.map(|home| {
            home.join(".config")
                .join("ios-control")
                .join("host-preferences.json")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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

    #[test]
    fn load_invalid_preferences_file_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalid-preferences.json");
        std::fs::write(&path, "{not-json").unwrap();
        let store = HostPreferencesStore::new(path);

        let prefs = store.load().unwrap();
        assert_eq!(prefs, HostPreferences::default());
    }

    #[test]
    fn preferences_path_uses_linux_xdg_or_home_fallback() {
        let xdg = PathBuf::from("/tmp/xdg-home");
        let home = PathBuf::from("/tmp/home");

        let from_xdg =
            HostPreferencesStore::path_from_env(Some(&xdg), Some(&home), None).unwrap();
        assert_eq!(
            from_xdg,
            xdg.join("ios-control").join("host-preferences.json")
        );

        let from_home = HostPreferencesStore::path_from_env(None, Some(&home), None).unwrap();
        assert_eq!(
            from_home,
            home.join(".config")
                .join("ios-control")
                .join("host-preferences.json")
        );
    }
}
