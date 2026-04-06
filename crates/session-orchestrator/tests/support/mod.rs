use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static RUNTIME_ENV_LOCK: Mutex<()> = Mutex::new(());
#[allow(dead_code)]
static BLE_HELPER_COUNTER: AtomicU64 = AtomicU64::new(0);

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

#[allow(dead_code)]
pub fn write_ble_helper(probe: &str, prepare: &str, execute: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let counter = BLE_HELPER_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "ios-control-session-orchestrator-ble-helper-{}-{nanos}-{counter}.sh",
        std::process::id()
    ));
    let probe_tag = format!("IOS_CONTROL_BLE_HELPER_PROBE_{nanos}_{counter}");
    let prepare_tag = format!("IOS_CONTROL_BLE_HELPER_PREPARE_{nanos}_{counter}");
    let execute_tag = format!("IOS_CONTROL_BLE_HELPER_EXECUTE_{nanos}_{counter}");
    let body = format!(
        r#"#!/bin/sh
case "$1" in
  probe)
    cat <<'{probe_tag}'
{probe}
{probe_tag}
    ;;
  prepare)
    cat <<'{prepare_tag}'
{prepare}
{prepare_tag}
    ;;
  status)
    cat <<'{prepare_tag}'
{prepare}
{prepare_tag}
    ;;
  execute)
    cat <<'{execute_tag}'
{execute}
{execute_tag}
    ;;
  stop)
    printf '%s\n' '{{"ok":true,"message":"helper stopped"}}'
    ;;
  forget-bond)
    printf '%s\n' '{{"ok":true,"message":"bond forgotten"}}'
    ;;
  *)
    exit 2
    ;;
esac
"#
    );
    fs::write(&path, body).expect("failed to write BLE helper script");
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&path)
            .expect("missing BLE helper metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("failed to make BLE helper executable");
    }
    path
}

#[allow(dead_code)]
pub fn write_direct_helper(body: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ios-control-session-orchestrator-direct-helper-{}-{nanos}.sh",
        std::process::id()
    ));
    fs::write(&path, body).expect("failed to write direct helper script");
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&path)
            .expect("missing direct helper metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("failed to make direct helper executable");
    }
    path
}

pub fn prepare_window_runtime_env(workspace_root: &Path) -> EnvVarGuards {
    EnvVarGuards::new(vec![
        // Keep the window-mock tests on the fallback control path unless they
        // explicitly install a helper for the BLE backend.
        EnvVarGuard::set("IOS_CONTROL_BLE_HELPER_SUPPORTED", "0"),
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
