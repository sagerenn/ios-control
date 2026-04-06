use ble_helper::{backend::HostCapability, state::helper_state_from_capability};
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn unsupported_capability_maps_to_unavailable_state() {
    let capability = HostCapability::unsupported("bluetooth peripheral role not supported");

    let state = helper_state_from_capability(&capability, false);

    assert_eq!(state.phase, "Unavailable");
    assert!(!state.execute_ready);
    assert_eq!(state.notes, vec!["bluetooth peripheral role not supported"]);
}

#[test]
fn supported_capability_without_bond_maps_to_ready_to_advertise() {
    let capability = HostCapability::supported("linux");

    let state = helper_state_from_capability(&capability, false);

    assert_eq!(state.phase, "ReadyToAdvertise");
    assert_eq!(
        state.checklist,
        vec!["Enable Bluetooth", "Pair the device when it appears"]
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_probe_reports_missing_system_bus() {
    let _guard = ENV_LOCK.lock().unwrap();
    let old_system_bus = std::env::var_os("IOS_CONTROL_BLE_TEST_SYSTEM_BUS");
    let old_adapter = std::env::var_os("IOS_CONTROL_BLE_TEST_ADAPTER");

    std::env::set_var("IOS_CONTROL_BLE_TEST_SYSTEM_BUS", "0");
    std::env::set_var("IOS_CONTROL_BLE_TEST_ADAPTER", "1");

    let capability = ble_helper::probe_host_capability();

    assert!(!capability.supported);
    assert_eq!(capability.reason.as_deref(), Some("system bus socket missing"));

    match old_system_bus {
        Some(value) => std::env::set_var("IOS_CONTROL_BLE_TEST_SYSTEM_BUS", value),
        None => std::env::remove_var("IOS_CONTROL_BLE_TEST_SYSTEM_BUS"),
    }
    match old_adapter {
        Some(value) => std::env::set_var("IOS_CONTROL_BLE_TEST_ADAPTER", value),
        None => std::env::remove_var("IOS_CONTROL_BLE_TEST_ADAPTER"),
    }
}
