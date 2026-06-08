use plugin_control_ble::helper_bridge::{BleHelperExecution, BleHelperProbe};
use plugin_control_ble::helper_config::probe_ble_helper;
use plugin_control_ble::linux_backend::{probe_linux_backend, LinuxProbeResult};
use std::env;
use std::path::PathBuf;
use std::sync::Mutex;
#[cfg(unix)]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    time::{SystemTime, UNIX_EPOCH},
};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn linux_probe_marks_unsupported_when_bluez_service_missing() {
    let probe = LinuxProbeResult::from_service_name(None);

    assert!(!probe.supported);
    assert_eq!(probe.reason.as_deref(), Some("org.bluez not available"));
}

#[test]
fn linux_probe_reports_reason_when_system_bus_missing() {
    let probe = LinuxProbeResult::from_runtime_checks(false, true, true);

    assert!(!probe.supported);
    assert_eq!(probe.reason.as_deref(), Some("system bus socket missing"));
}

#[test]
fn linux_runtime_probe_is_conservative_without_runtime_signals() {
    let _guard = ENV_LOCK.lock().unwrap();
    let old_service = env::var_os("IOS_CONTROL_BLUEZ_SERVICE");
    env::remove_var("IOS_CONTROL_BLUEZ_SERVICE");

    let probe = probe_linux_backend();
    if cfg!(target_os = "linux") {
        assert!(!probe.supported || probe.reason.is_none());
    } else {
        assert!(!probe.supported);
    }

    match old_service {
        Some(value) => env::set_var("IOS_CONTROL_BLUEZ_SERVICE", value),
        None => env::remove_var("IOS_CONTROL_BLUEZ_SERVICE"),
    }
}

#[test]
fn ble_probe_reports_helper_backed_transport() {
    let capability = probe_ble_helper(None);
    assert!(!capability.supported);
    assert_eq!(
        capability.reason.as_deref(),
        Some("ble helper not found in override, sibling binary, or bundled helpers directory")
    );
}

#[test]
fn ble_helper_probe_requires_prepare_and_execute_support() {
    let probe: BleHelperProbe = serde_json::from_str(
        r#"{"supported":true,"supports_prepare":true,"supports_execute":true,"supports_status":true,"supports_stop":true,"supports_forget_bond":true}"#,
    )
    .unwrap();

    assert!(probe.supported);
    assert!(probe.supports_prepare);
    assert!(probe.supports_execute);
}

#[test]
fn ble_helper_execution_roundtrips_success() {
    let execution: BleHelperExecution =
        serde_json::from_str(r#"{"phase":"Succeeded","summary":"helper executed"}"#).unwrap();

    assert_eq!(execution.phase, "Succeeded");
    assert_eq!(execution.summary, "helper executed");
}

#[cfg(unix)]
fn write_test_helper_script(name: &str, body: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = env::temp_dir().join(format!(
        "ios-control-{name}-{}-{nanos}.sh",
        std::process::id()
    ));
    fs::write(&path, body).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
    path
}

#[cfg(unix)]
#[test]
fn ble_probe_times_out_when_helper_hangs() {
    let _guard = ENV_LOCK.lock().unwrap();
    let old_timeout = env::var_os("IOS_CONTROL_BLE_HELPER_TIMEOUT_MS");
    env::set_var("IOS_CONTROL_BLE_HELPER_TIMEOUT_MS", "50");
    let helper = write_test_helper_script(
        "ble-timeout",
        "#!/bin/sh\nsleep 1\nprintf '%s\\n' '{\"supported\":true,\"supports_prepare\":true,\"supports_execute\":true,\"supports_status\":true,\"supports_stop\":true,\"supports_forget_bond\":true}'\n",
    );

    let capability = probe_ble_helper(Some(helper.clone()));
    assert!(!capability.supported);
    assert!(capability
        .reason
        .as_deref()
        .unwrap_or_default()
        .contains("timed out"));

    let _ = fs::remove_file(helper);
    match old_timeout {
        Some(value) => env::set_var("IOS_CONTROL_BLE_HELPER_TIMEOUT_MS", value),
        None => env::remove_var("IOS_CONTROL_BLE_HELPER_TIMEOUT_MS"),
    }
}

#[cfg(unix)]
#[test]
fn ble_probe_handles_chatty_helper_output_without_timeout() {
    let _guard = ENV_LOCK.lock().unwrap();
    let old_timeout = env::var_os("IOS_CONTROL_BLE_HELPER_TIMEOUT_MS");
    env::set_var("IOS_CONTROL_BLE_HELPER_TIMEOUT_MS", "2000");
    let helper = write_test_helper_script(
        "ble-chatty",
        "#!/bin/sh\ni=0\nwhile [ \"$i\" -lt 20000 ]; do\n  printf 'noise-%s\\n' \"$i\" 1>&2\n  i=$((i + 1))\ndone\nprintf '%s\\n' '{\"supported\":true,\"supports_prepare\":true,\"supports_execute\":true,\"supports_status\":true,\"supports_stop\":true,\"supports_forget_bond\":true}'\n",
    );

    let capability = probe_ble_helper(Some(helper.clone()));
    assert!(capability.supported);
    assert_eq!(capability.reason, None);

    let _ = fs::remove_file(helper);
    match old_timeout {
        Some(value) => env::set_var("IOS_CONTROL_BLE_HELPER_TIMEOUT_MS", value),
        None => env::remove_var("IOS_CONTROL_BLE_HELPER_TIMEOUT_MS"),
    }
}

#[cfg(unix)]
#[test]
fn ble_probe_uses_helper_reason_when_helper_reports_unsupported() {
    let helper = write_test_helper_script(
        "ble-unsupported",
        "#!/bin/sh\nprintf '%s\\n' '{\"supported\":false,\"reason\":\"org.bluez not available\",\"supports_prepare\":true,\"supports_execute\":true,\"supports_status\":true,\"supports_stop\":true,\"supports_forget_bond\":true}'\n",
    );

    let capability = probe_ble_helper(Some(helper.clone()));
    assert!(!capability.supported);
    assert_eq!(
        capability.reason.as_deref(),
        Some("org.bluez not available")
    );

    let _ = fs::remove_file(helper);
}
