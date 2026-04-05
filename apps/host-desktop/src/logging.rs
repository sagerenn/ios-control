use anyhow::Result;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::preferences::HostPreferencesStore;

#[derive(Debug, Clone)]
pub struct HostLogWriter {
    path: PathBuf,
}

impl HostLogWriter {
    pub fn from_preferences_path(preferences_path: &Path) -> Result<Self> {
        let directory = HostPreferencesStore::log_dir_for_preferences_path(preferences_path);
        std::fs::create_dir_all(&directory)?;

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = directory.join(format!(
            "host-desktop-{}-{}.log",
            nonce,
            std::process::id()
        ));

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        writeln!(file, "host-desktop launch pid={}", std::process::id())?;
        file.flush()?;

        Ok(Self { path })
    }

    pub fn append_line(&self, line: &str) -> Result<()> {
        let mut file = std::fs::OpenOptions::new().append(true).open(&self.path)?;
        writeln!(file, "{line}")?;
        file.flush()?;
        Ok(())
    }
}
