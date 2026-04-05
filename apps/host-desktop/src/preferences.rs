use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
            Ok(text) => match serde_json::from_str(&text) {
                Ok(prefs) => Ok(prefs),
                Err(err) => {
                    eprintln!(
                        "warning: invalid host preferences JSON at {}: {}",
                        self.path.display(),
                        err
                    );
                    Ok(HostPreferences::default())
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Ok(HostPreferences::default())
            }
            Err(err) => {
                eprintln!(
                    "warning: failed to read host preferences at {}: {}",
                    self.path.display(),
                    err
                );
                Ok(HostPreferences::default())
            }
        }
    }

    pub fn save(&self, prefs: &HostPreferences) -> Result<()> {
        if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(prefs)?;

        let parent_dir = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let file_name = self.path.file_name().ok_or_else(|| {
            anyhow!(
                "host preferences path is missing file name: {}",
                self.path.display()
            )
        })?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let tmp_path = parent_dir.join(format!(".{}.{}.tmp", file_name.to_string_lossy(), nonce));

        let write_result = (|| -> Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp_path)?;
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
            Ok(())
        })();

        if let Err(err) = write_result {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(err);
        }

        if let Err(rename_err) = std::fs::rename(&tmp_path, &self.path) {
            #[cfg(target_os = "windows")]
            {
                if self.path.exists() {
                    std::fs::remove_file(&self.path)?;
                    std::fs::rename(&tmp_path, &self.path)?;
                    return Ok(());
                }
            }
            let _ = std::fs::remove_file(&tmp_path);
            return Err(rename_err.into());
        }

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
            Self::path_from_env(xdg.as_deref(), home.as_deref(), None)
        }
    }

    pub fn path_from_env(
        xdg_config_home: Option<&Path>,
        home: Option<&Path>,
        appdata: Option<&Path>,
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

        let from_xdg = HostPreferencesStore::path_from_env(
            Some(xdg.as_path()),
            Some(home.as_path()),
            None,
        )
        .unwrap();
        assert_eq!(
            from_xdg,
            xdg.join("ios-control").join("host-preferences.json")
        );

        let from_home =
            HostPreferencesStore::path_from_env(None, Some(home.as_path()), None).unwrap();
        assert_eq!(
            from_home,
            home.join(".config")
                .join("ios-control")
                .join("host-preferences.json")
        );
    }

    #[test]
    fn save_relative_path_without_parent_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let run = || -> Result<()> {
            let store = HostPreferencesStore::new(PathBuf::from("host-preferences.json"));
            let prefs = HostPreferences {
                selected_device_id: Some("device-1".into()),
                selected_source_id: Some("window-helper-1".into()),
            };
            store.save(&prefs)?;
            Ok(())
        };

        let result = run();
        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }
}
