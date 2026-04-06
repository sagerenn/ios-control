#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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

    pub fn remove(key: &'static str) -> Self {
        let original = std::env::var_os(key);
        std::env::remove_var(key);
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
    RUNTIME_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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
        capture_direct: plugin_path(workspace_root, "plugin-capture-direct"),
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
            "plugin-capture-direct",
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

pub fn write_preferences_json(json: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "host-desktop-test-preferences-{}-{}.json",
        std::process::id(),
        nonce
    ));
    std::fs::write(&path, json).expect("preferences json should be written");
    path
}

pub fn write_direct_helper(body: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "host-desktop-direct-helper-{}-{}.sh",
        std::process::id(),
        nonce
    ));
    std::fs::write(&path, body).expect("direct helper script should be written");
    #[cfg(unix)]
    {
        let mut perms = std::fs::metadata(&path)
            .expect("direct helper metadata should exist")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms)
            .expect("direct helper script should be executable");
    }
    path
}

pub struct StagedBundleLayout {
    pub _tempdir: tempfile::TempDir,
    pub root: PathBuf,
    pub host_exe: PathBuf,
    pub plugins_dir: PathBuf,
}

pub fn stage_bundle_layout() -> StagedBundleLayout {
    let tempdir = tempfile::tempdir().expect("bundle tempdir should be created");
    let root = tempdir.path().join("ios-control-x86_64-pc-windows-msvc");
    let bin_dir = root.join("bin");
    let plugins_dir = root.join("plugins");
    std::fs::create_dir_all(&bin_dir).expect("bundle bin dir should exist");
    std::fs::create_dir_all(&plugins_dir).expect("bundle plugins dir should exist");

    let host_exe = bin_dir.join(format!("host-desktop{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&host_exe, b"host").expect("bundle host exe should be stubbed");

    for name in [
        "plugin-capture-window",
        "plugin-capture-direct",
        "plugin-control-ble",
        "plugin-control-window-bridge",
        "plugin-grounding-core",
    ] {
        std::fs::write(
            plugins_dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
            name.as_bytes(),
        )
        .expect("bundle plugin should be stubbed");
    }

    StagedBundleLayout {
        _tempdir: tempdir,
        root,
        host_exe,
        plugins_dir,
    }
}
