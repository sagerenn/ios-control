use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[cfg(target_os = "linux")]
#[test]
fn prepare_reports_unavailable_when_capability_probe_fails() {
    let _guard = ENV_LOCK.lock().unwrap();
    let old_system_bus = std::env::var_os("IOS_CONTROL_BLE_TEST_SYSTEM_BUS");
    let old_adapter = std::env::var_os("IOS_CONTROL_BLE_TEST_ADAPTER");
    let old_state_dir = std::env::var_os("IOS_CONTROL_BLE_HELPER_STATE_DIR");

    let state_dir =
        std::env::temp_dir().join(format!("ios-control-ble-helper-cli-{}", std::process::id()));

    std::env::set_var("IOS_CONTROL_BLE_TEST_SYSTEM_BUS", "0");
    std::env::set_var("IOS_CONTROL_BLE_TEST_ADAPTER", "1");
    std::env::set_var("IOS_CONTROL_BLE_HELPER_STATE_DIR", &state_dir);

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ble-helper"))
        .arg("prepare")
        .output()
        .unwrap();

    assert!(output.status.success());
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["phase"], "Unavailable");
    assert_eq!(payload["notes"][0], "system bus socket missing");
    assert_eq!(
        payload["checklist"][0],
        "Use fallback control or install supported Bluetooth support"
    );

    let _ = std::fs::remove_dir_all(state_dir);
    match old_system_bus {
        Some(value) => std::env::set_var("IOS_CONTROL_BLE_TEST_SYSTEM_BUS", value),
        None => std::env::remove_var("IOS_CONTROL_BLE_TEST_SYSTEM_BUS"),
    }
    match old_adapter {
        Some(value) => std::env::set_var("IOS_CONTROL_BLE_TEST_ADAPTER", value),
        None => std::env::remove_var("IOS_CONTROL_BLE_TEST_ADAPTER"),
    }
    match old_state_dir {
        Some(value) => std::env::set_var("IOS_CONTROL_BLE_HELPER_STATE_DIR", value),
        None => std::env::remove_var("IOS_CONTROL_BLE_HELPER_STATE_DIR"),
    }
}
