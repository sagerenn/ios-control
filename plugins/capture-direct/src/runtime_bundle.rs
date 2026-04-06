use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub const DIRECT_RUNTIME_ROOT_ENV: &str = "IOS_CONTROL_DIRECT_RUNTIME_ROOT";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectRuntimeBundle {
    pub root: PathBuf,
    pub manifest_path: PathBuf,
    pub uxplay_path: PathBuf,
}

impl DirectRuntimeBundle {
    pub fn configured_root() -> Option<PathBuf> {
        std::env::var_os(DIRECT_RUNTIME_ROOT_ENV).map(PathBuf::from)
    }

    pub fn resolve() -> Result<Self> {
        let root = Self::configured_root()
            .ok_or_else(|| anyhow!("{DIRECT_RUNTIME_ROOT_ENV} not configured"))?;
        let manifest_path = root.join("manifest.json");
        if !manifest_path.is_file() {
            return Err(anyhow!(
                "direct runtime manifest missing: {}",
                manifest_path.display()
            ));
        }

        let uxplay_path = root.join(format!("uxplay{}", std::env::consts::EXE_SUFFIX));
        if !uxplay_path.is_file() {
            return Err(anyhow!("uxplay binary missing: {}", uxplay_path.display()));
        }

        Ok(Self {
            root,
            manifest_path,
            uxplay_path,
        })
    }

    pub fn probe(&self) -> Result<()> {
        let status = Command::new(&self.uxplay_path)
            .arg("--help")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("uxplay probe failed with status {status}"))
        }
    }
}
