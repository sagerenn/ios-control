use plugin_control_ble::helper_bridge::{run_execute, run_prepare, BleHelperExecution, BleHelperPrepare};

#[cfg(unix)]
use std::{
    env, fs, os::unix::fs::PermissionsExt, path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
static HELPER_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(unix)]
fn write_ble_helper(probe: &str, prepare: &str, execute: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = HELPER_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = env::temp_dir().join(format!(
        "ios-control-ble-helper-{}-{nanos}-{counter}.sh",
        std::process::id(),
    ));
    let body = format!(
        r#"#!/bin/sh
case "$1" in
  probe)
    printf '%s\n' '{probe}'
    ;;
  prepare)
    printf '%s\n' '{prepare}'
    ;;
  execute)
    printf '%s\n' '{execute}'
    ;;
  *)
    exit 2
    ;;
esac
"#
    );
    fs::write(&path, body).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
    path
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
        r#"{"phase":"Advertising","checklist":["Enable Bluetooth"],"notes":["Waiting for iPhone"]}"#,
    )
    .unwrap();

    assert_eq!(prepare.phase, "Advertising");
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
