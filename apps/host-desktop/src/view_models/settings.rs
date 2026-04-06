use std::path::Path;

use crate::preferences::HostPreferencesStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsViewModel {
    pub rows: Vec<String>,
}

impl SettingsViewModel {
    pub fn empty() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn from_preferences_path(path: Option<&Path>) -> Self {
        let Some(path) = path else {
            return Self::empty();
        };

        let log_dir = HostPreferencesStore::log_dir_for_preferences_path(path);
        Self {
            rows: vec![
                format!("Preferences: {}", path.display()),
                format!("Logs: {}", log_dir.display()),
            ],
        }
    }
}
