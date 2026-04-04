#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

use ios_control_frame_transport::FrameSlot;
use ios_control_session_orchestrator::PluginPaths;

static RUNTIME_ENV_LOCK: Mutex<()> = Mutex::new(());

pub struct EnvVarGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    pub fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let original = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, original }
    }
}

pub struct EnvVarGuards {
    _guards: Vec<EnvVarGuard>,
}

impl EnvVarGuards {
    pub fn new(guards: Vec<EnvVarGuard>) -> Self {
        Self { _guards: guards }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(value) = self.original.take() {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

pub fn runtime_env_lock() -> MutexGuard<'static, ()> {
    RUNTIME_ENV_LOCK.lock().expect("runtime env lock poisoned")
}

pub fn target_dir(workspace_root: &Path) -> PathBuf {
    let mut base = match std::env::var_os("CARGO_TARGET_DIR") {
        Some(path) => {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                workspace_root.join(path)
            }
        }
        None => workspace_root.join("target"),
    };

    if let Some(target) = std::env::var_os("CARGO_BUILD_TARGET") {
        base.push(target);
    }

    base
}

pub fn plugin_path(workspace_root: &Path, name: &str) -> PathBuf {
    target_dir(workspace_root).join(format!("debug/{}{}", name, std::env::consts::EXE_SUFFIX))
}

pub fn host_plugin_paths(workspace_root: &Path) -> PluginPaths {
    PluginPaths {
        capture: plugin_path(workspace_root, "plugin-capture-window"),
        control_ble: plugin_path(workspace_root, "plugin-control-ble"),
        control_fallback: plugin_path(workspace_root, "plugin-control-window-bridge"),
        grounding: Some(plugin_path(workspace_root, "plugin-grounding-core")),
    }
}

pub fn build_plugins(workspace_root: &Path) {
    let output = Command::new("cargo")
        .args([
            "build",
            "-p",
            "plugin-capture-window",
            "-p",
            "plugin-control-ble",
            "-p",
            "plugin-control-window-bridge",
            "-p",
            "plugin-grounding-core",
        ])
        .current_dir(workspace_root)
        .output()
        .expect("failed to invoke cargo build for mock plugins");

    assert!(
        output.status.success(),
        "cargo build for mock plugins failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn prepare_window_runtime_env(workspace_root: &Path) -> EnvVarGuards {
    EnvVarGuards::new(vec![
        EnvVarGuard::set(
            "IOS_CONTROL_WINDOW_CAPTURE_HELPER",
            plugin_path(workspace_root, "plugin-capture-window"),
        ),
        EnvVarGuard::set(
            "IOS_CONTROL_WINDOW_INPUT_HELPER",
            plugin_path(workspace_root, "plugin-control-window-bridge"),
        ),
    ])
}

pub fn write_slot_bytes(bytes: &[u8]) -> String {
    let mut slot = FrameSlot::new(bytes.len()).expect("slot should be created");
    slot.write(bytes).expect("slot bytes should be written");
    let path = slot.path().display().to_string();
    std::mem::forget(slot);
    path
}
