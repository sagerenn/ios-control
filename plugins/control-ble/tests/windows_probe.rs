use plugin_control_ble::windows_backend::{probe_windows_backend, WindowsProbeResult};
use std::env;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn windows_probe_marks_unsupported_without_peripheral_role() {
    let probe = WindowsProbeResult::from_peripheral_role(false);

    assert!(!probe.supported);
    assert_eq!(
        probe.reason.as_deref(),
        Some("bluetooth peripheral role not supported")
    );
}

#[test]
fn windows_probe_reports_reason_when_radio_missing() {
    let probe = WindowsProbeResult::from_runtime_checks(false, false);

    assert!(!probe.supported);
    assert_eq!(
        probe.reason.as_deref(),
        Some("bluetooth radio not detected")
    );
}

#[test]
fn windows_runtime_probe_is_conservative_without_runtime_signals() {
    let _guard = ENV_LOCK.lock().unwrap();
    let old_radio = env::var_os("IOS_CONTROL_BLE_RADIO_PRESENT");
    let old_role = env::var_os("IOS_CONTROL_BLE_PERIPHERAL_ROLE");
    env::remove_var("IOS_CONTROL_BLE_RADIO_PRESENT");
    env::remove_var("IOS_CONTROL_BLE_PERIPHERAL_ROLE");

    let probe = probe_windows_backend();
    assert!(!probe.supported);

    match old_radio {
        Some(value) => env::set_var("IOS_CONTROL_BLE_RADIO_PRESENT", value),
        None => env::remove_var("IOS_CONTROL_BLE_RADIO_PRESENT"),
    }
    match old_role {
        Some(value) => env::set_var("IOS_CONTROL_BLE_PERIPHERAL_ROLE", value),
        None => env::remove_var("IOS_CONTROL_BLE_PERIPHERAL_ROLE"),
    }
}
