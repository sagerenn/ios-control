use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_DIRECT_PREVIEW_FPS: u32 = 20;
pub const MIN_DIRECT_PREVIEW_FPS: u32 = 5;
pub const MAX_DIRECT_PREVIEW_FPS: u32 = 60;
pub const DEFAULT_DIRECT_PREVIEW_HEIGHT: u32 = 1280;
pub const MIN_DIRECT_PREVIEW_HEIGHT: u32 = 540;
pub const MAX_DIRECT_PREVIEW_HEIGHT: u32 = 1920;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownDevicePreference {
    pub known_device_id: String,
    pub display_name: String,
    #[serde(default)]
    pub stable_id: Option<String>,
    #[serde(default)]
    pub last_source_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPreferences {
    pub selected_device_id: Option<String>,
    pub selected_source_id: Option<String>,
    #[serde(default)]
    pub ble_pointer_long_axis_units: Option<u32>,
    #[serde(default)]
    pub direct_preview_fps: Option<u32>,
    #[serde(default)]
    pub direct_preview_height: Option<u32>,
    #[serde(default)]
    pub known_devices: Vec<KnownDevicePreference>,
}

impl HostPreferences {
    pub fn direct_preview_fps(&self) -> u32 {
        self.direct_preview_fps
            .unwrap_or(DEFAULT_DIRECT_PREVIEW_FPS)
            .clamp(MIN_DIRECT_PREVIEW_FPS, MAX_DIRECT_PREVIEW_FPS)
    }

    pub fn direct_preview_height(&self) -> u32 {
        self.direct_preview_height
            .unwrap_or(DEFAULT_DIRECT_PREVIEW_HEIGHT)
            .clamp(MIN_DIRECT_PREVIEW_HEIGHT, MAX_DIRECT_PREVIEW_HEIGHT)
    }

    pub fn direct_preview_width(&self) -> u32 {
        direct_preview_width_for_height(self.direct_preview_height())
    }
}

pub fn direct_preview_width_for_height(height: u32) -> u32 {
    ((u64::from(height) * 9 + 8) / 16).max(1) as u32
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

    pub fn log_dir_for_preferences_path(path: &Path) -> PathBuf {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .join("logs")
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

    pub fn path(&self) -> &Path {
        &self.path
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
            ble_pointer_long_axis_units: Some(120),
            direct_preview_fps: Some(20),
            direct_preview_height: Some(1280),
            known_devices: Vec::new(),
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
            HostPreferencesStore::path_from_env(Some(xdg.as_path()), Some(home.as_path()), None)
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
    fn log_directory_is_sibling_to_preferences_file() {
        let prefs = PathBuf::from("/tmp/app/ios-control/host-preferences.json");
        assert_eq!(
            HostPreferencesStore::log_dir_for_preferences_path(&prefs),
            PathBuf::from("/tmp/app/ios-control/logs")
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
                ble_pointer_long_axis_units: None,
                direct_preview_fps: None,
                direct_preview_height: None,
                known_devices: Vec::new(),
            };
            store.save(&prefs)?;
            Ok(())
        };

        let result = run();
        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }
}
