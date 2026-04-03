use plugin_control_ble::linux_backend::{probe_linux_backend, LinuxProbeResult};
use plugin_control_ble::helper_config::probe_ble_helper;
use std::env;
use std::sync::Mutex;

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
        Some("IOS_CONTROL_BLE_HELPER not configured")
    );
}
