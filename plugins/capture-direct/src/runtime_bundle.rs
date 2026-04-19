use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub const DIRECT_RUNTIME_ROOT_ENV: &str = "IOS_CONTROL_DIRECT_RUNTIME_ROOT";
pub const RUNTIME_ROOT_ENV: &str = DIRECT_RUNTIME_ROOT_ENV;
pub const BLE_PATH_ENV: &str = "IOS_CONTROL_DIRECT_BLE_PATH";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectRuntimeBundle {
    pub root: PathBuf,
    pub manifest_path: PathBuf,
    pub uxplay_path: PathBuf,
    pub gst_launch_path: PathBuf,
    pub beacon_helper_path: PathBuf,
    pub beacon_script_path: PathBuf,
    pub python_path: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DirectRuntimeManifest {
    uxplay_path: Option<String>,
    gst_launch_path: Option<String>,
    beacon_helper_path: Option<String>,
    beacon_script_path: Option<String>,
    python_path: Option<String>,
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

        let manifest: DirectRuntimeManifest =
            serde_json::from_slice(&std::fs::read(&manifest_path)?)?;

        let uxplay_path = root.join(
            manifest
                .uxplay_path
                .unwrap_or_else(|| format!("uxplay{}", std::env::consts::EXE_SUFFIX)),
        );
        if !uxplay_path.is_file() {
            return Err(anyhow!("uxplay binary missing: {}", uxplay_path.display()));
        }

        let gst_launch_path = root.join(
            manifest
                .gst_launch_path
                .unwrap_or_else(|| format!("gst-launch-1.0{}", std::env::consts::EXE_SUFFIX)),
        );
        if !gst_launch_path.is_file() {
            return Err(anyhow!(
                "gst-launch binary missing: {}",
                gst_launch_path.display()
            ));
        }

        let beacon_helper_path = root.join(
            manifest.beacon_helper_path.unwrap_or_else(|| {
                format!("beacon-helper{}", std::env::consts::EXE_SUFFIX)
            }),
        );
        if !beacon_helper_path.is_file() {
            return Err(anyhow!(
                "beacon helper missing: {}",
                beacon_helper_path.display()
            ));
        }

        let beacon_script_path = root.join(
            manifest
                .beacon_script_path
                .unwrap_or_else(|| "Bluetooth_LE_beacon/uxplay-beacon.py".into()),
        );
        if !beacon_script_path.is_file() {
            return Err(anyhow!(
                "beacon script missing: {}",
                beacon_script_path.display()
            ));
        }

        let python_path = manifest.python_path.unwrap_or_else(|| {
            if cfg!(target_os = "windows") {
                "py".into()
            } else {
                "python3".into()
            }
        });

        Ok(Self {
            root,
            manifest_path,
            uxplay_path,
            gst_launch_path,
            beacon_helper_path,
            beacon_script_path,
            python_path,
        })
    }

    pub fn probe(&self) -> Result<()> {
        let mut command = Command::new(&self.uxplay_path);
        command
            .arg("--help")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        self.apply_runtime_env(&mut command);
        let status = command.status()?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("uxplay probe failed with status {status}"))
        }
    }

    pub fn apply_runtime_env(&self, command: &mut Command) {
        let gst_root = self.root.join("gstreamer");
        if cfg!(target_os = "windows") {
            let gst_bin = gst_root.join("bin");
            if gst_bin.is_dir() {
                let path = std::env::var_os("PATH").unwrap_or_default();
                let mut composed = gst_bin.into_os_string();
                if !path.is_empty() {
                    composed.push(";");
                    composed.push(path);
                }
                command.env("PATH", composed);
            }
            let gst_plugins = gst_root.join("plugins");
            if gst_plugins.is_dir() {
                command.env("GST_PLUGIN_PATH_1_0", gst_plugins);
            }
        } else {
            let gst_plugins = gst_root.join("plugins");
            if gst_plugins.is_dir() {
                command.env("GST_PLUGIN_PATH", gst_plugins);
            }
            let gst_lib = gst_root.join("lib");
            if gst_lib.is_dir() {
                command.env("LD_LIBRARY_PATH", gst_lib);
            }
        }
    }
}
