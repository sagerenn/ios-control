use std::path::PathBuf;
use std::sync::Mutex;

use host_desktop::inventory::providers::bluetooth::discover_bluetooth_devices;
use host_desktop::preferences::HostPreferences;
use ios_control_session_orchestrator::PluginPaths;

static BLUETOOTH_OVERRIDE_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn bluetooth_provider_collapses_multiple_windows_records_for_one_phone() {
    let _guard = BLUETOOTH_OVERRIDE_LOCK.lock().unwrap();
    std::env::set_var(
        "IOS_CONTROL_TEST_BLUETOOTH_DEVICES_JSON",
        r#"[
            {"stable_id":"BTHENUM\\DEV_A","display_name":"Alice iPhone","container_id":"{PHONE-1}"},
            {"stable_id":"BTHENUM\\DEV_B","display_name":"Alice iPhone","container_id":"{PHONE-1}"},
            {"stable_id":"BTHENUM\\DEV_C","display_name":"Alice iPhone AVRCP Transport","container_id":"{PHONE-1}"},
            {"stable_id":"BTHENUM\\DEV_D","display_name":"Alice iPhone AVRCP Transport","container_id":"{PHONE-1}"}
        ]"#,
    );

    let devices = discover_bluetooth_devices(&plugin_paths(), &HostPreferences::default());

    std::env::remove_var("IOS_CONTROL_TEST_BLUETOOTH_DEVICES_JSON");

    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].display_name, "Alice iPhone");
}

fn plugin_paths() -> PluginPaths {
    PluginPaths {
        capture: PathBuf::from("/nonexistent/capture"),
        capture_direct: PathBuf::from("/nonexistent/capture-direct"),
        control_ble: PathBuf::from("/nonexistent/control-ble"),
        control_fallback: PathBuf::from("/nonexistent/control-fallback"),
        grounding: None,
    }
}
