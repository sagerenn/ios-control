use plugin_control_ble::helper_bridge::{
    run_execute, run_forget_bond, run_prepare, run_probe, run_status, run_stop, BleHelperAck,
    BleHelperExecution, BleHelperPrepare, BleHelperStatus,
};
use std::{
    env, fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

static HELPER_COUNTER: AtomicU64 = AtomicU64::new(0);

fn write_ble_helper_with_mode(
    probe: &str,
    prepare: &str,
    status: &str,
    execute: &str,
    executable: bool,
) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = HELPER_COUNTER.fetch_add(1, Ordering::Relaxed);
    let extension = if cfg!(windows) { "cmd" } else { "sh" };
    let path = env::temp_dir().join(format!(
        "ios-control-ble-helper-{}-{nanos}-{counter}.{extension}",
        std::process::id(),
    ));
    let body = if cfg!(windows) {
        format!(
            r#"@echo off
if "%~1"=="probe" (
  echo {probe}
) else if "%~1"=="prepare" (
  echo {prepare}
) else if "%~1"=="status" (
  echo {status}
) else if "%~1"=="execute" (
  echo {execute}
) else if "%~1"=="stop" (
  echo {{"ok":true,"message":"helper stopped"}}
) else if "%~1"=="forget-bond" (
  echo {{"ok":true,"message":"bond forgotten"}}
) else (
  exit /b 2
)
"#
        )
    } else {
        format!(
            r#"#!/bin/sh
case "$1" in
  probe)
    printf '%s\n' '{probe}'
    ;;
  prepare)
    printf '%s\n' '{prepare}'
    ;;
  status)
    printf '%s\n' '{status}'
    ;;
  execute)
    printf '%s\n' '{execute}'
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
        )
    };
    fs::write(&path, body).unwrap();
    #[cfg(unix)]
    if executable {
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }
    path
}

#[cfg(unix)]
fn write_ble_helper(probe: &str, prepare: &str, execute: &str) -> PathBuf {
    write_ble_helper_with_mode(probe, prepare, prepare, execute, true)
}

#[test]
fn ble_helper_probe_runs_shell_script_helpers() {
    let helper = write_ble_helper_with_mode(
        r#"{"supported":true,"supports_prepare":true,"supports_execute":true}"#,
        r#"{"phase":"Advertising","checklist":["Enable Bluetooth"],"notes":[]}"#,
        r#"{"phase":"Advertising","checklist":["Enable Bluetooth"],"notes":[]}"#,
        r#"{"phase":"Succeeded","summary":"tap applied","observed_change":true}"#,
        false,
    );

    let probe = run_probe(&helper).unwrap();
    assert!(probe.supported);
    assert!(probe.supports_prepare);
    assert!(probe.supports_execute);

    let _ = fs::remove_file(helper);
}

#[cfg(unix)]
#[test]
fn ble_helper_prepare_returns_control_phase_and_checklist() {
    let helper = write_ble_helper(
        r#"{"supported":true,"supports_prepare":true,"supports_execute":true}"#,
        r#"{"phase":"Advertising","checklist":["Enable Bluetooth","Pair the device"],"notes":["Waiting for iPhone"]}"#,
        r#"{"phase":"Succeeded","summary":"pointer action applied","observed_change":true}"#,
    );

    let prepare = run_prepare(&helper).unwrap();
    assert_eq!(prepare.phase, "Advertising");
    assert_eq!(prepare.checklist.len(), 2);

    let _ = fs::remove_file(helper);
}

#[cfg(unix)]
#[test]
fn ble_helper_execute_exposes_observed_change() {
    let helper = write_ble_helper(
        r#"{"supported":true,"supports_prepare":true,"supports_execute":true}"#,
        r#"{"phase":"Connected","checklist":[],"notes":[]}"#,
        r#"{"phase":"Succeeded","summary":"tap applied","observed_change":true}"#,
    );

    let execution = run_execute(&helper, "pointer").unwrap();
    assert!(execution.observed_change);

    let _ = fs::remove_file(helper);
}

#[test]
fn ble_helper_prepare_roundtrips_json() {
    let prepare: BleHelperPrepare = serde_json::from_str(
        r#"{"phase":"ReconnectPending","checklist":["Enable Bluetooth"],"notes":["Waiting for iPhone"]}"#,
    )
    .unwrap();

    assert_eq!(prepare.phase, "ReconnectPending");
    assert_eq!(prepare.checklist, vec!["Enable Bluetooth"]);
    assert_eq!(prepare.notes, vec!["Waiting for iPhone"]);
}

#[test]
fn ble_helper_execution_roundtrips_observed_change() {
    let execution: BleHelperExecution = serde_json::from_str(
        r#"{"phase":"Succeeded","summary":"tap applied","observed_change":true}"#,
    )
    .unwrap();

    assert_eq!(execution.phase, "Succeeded");
    assert_eq!(execution.summary, "tap applied");
    assert!(execution.observed_change);
}

#[test]
fn ble_helper_status_roundtrips_json() {
    let status: BleHelperStatus = serde_json::from_str(
        r#"{"phase":"BondedIdle","checklist":["Reconnect device"],"notes":["Stored bond available"],"paired_device_id":"device-1","paired_device_name":"Alice iPhone","bonded":true,"execute_ready":false}"#,
    )
    .unwrap();

    assert_eq!(status.phase, "BondedIdle");
    assert_eq!(status.paired_device_id.as_deref(), Some("device-1"));
    assert_eq!(status.paired_device_name.as_deref(), Some("Alice iPhone"));
    assert!(status.bonded);
    assert!(!status.execute_ready);
}

#[cfg(unix)]
#[test]
fn ble_helper_status_stop_and_forget_bond_use_helper_commands() {
    let helper = write_ble_helper(
        r#"{"supported":true,"supports_prepare":true,"supports_execute":true,"supports_status":true,"supports_stop":true,"supports_forget_bond":true}"#,
        r#"{"phase":"Connected","checklist":[],"notes":[]}"#,
        r#"{"phase":"Succeeded","summary":"tap applied","observed_change":true}"#,
    );

    let status = run_status(&helper).unwrap();
    assert_eq!(status.phase, "Connected");

    let stop = run_stop(&helper).unwrap();
    assert_eq!(
        stop,
        BleHelperAck {
            ok: true,
            message: Some("helper stopped".into())
        }
    );

    let forget = run_forget_bond(&helper, "device-1").unwrap();
    assert_eq!(
        forget,
        BleHelperAck {
            ok: true,
            message: Some("bond forgotten".into())
        }
    );

    let _ = fs::remove_file(helper);
}
