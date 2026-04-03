use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{
    BackendSelection, DeviceSessionStatus, DeviceSessionSummary, SessionPhase, SessionSubstate,
};

#[test]
fn device_session_summary_defaults_to_disconnected() {
    let summary = DeviceSessionSummary::new("device-1".into(), "iPhone 15".into());

    assert_eq!(summary.phase, SessionPhase::Disconnected);
    assert_eq!(summary.plugin_health, PluginHealth::Unknown);
    assert!(summary.capture_plugin.is_none());
    assert!(summary.control_plugin.is_none());
    assert!(summary.grounding_plugin.is_none());
}

#[test]
fn device_session_status_roundtrips_operator_state() {
    let status = DeviceSessionStatus::new(
        DeviceSessionSummary {
            device_id: "device-1".into(),
            device_name: "Operator iPhone".into(),
            phase: SessionPhase::Streaming,
            plugin_health: PluginHealth::Healthy,
            capture_plugin: Some("capture.window".into()),
            control_plugin: Some("control.ble".into()),
            grounding_plugin: Some("grounding.core".into()),
        },
        SessionSubstate::ControlReady,
        BackendSelection {
            capture_backend: "capture.window".into(),
            control_backend: "control.ble".into(),
        },
        None,
    )
    .unwrap();

    let json = serde_json::to_string(&status).unwrap();
    let decoded: DeviceSessionStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, status);
}

#[test]
fn device_session_status_rejects_mismatched_summary_phase() {
    let error = DeviceSessionStatus::new(
        DeviceSessionSummary {
            device_id: "device-2".into(),
            device_name: "Broken State".into(),
            phase: SessionPhase::Streaming,
            plugin_health: PluginHealth::Healthy,
            capture_plugin: Some("capture.window".into()),
            control_plugin: Some("control.ble".into()),
            grounding_plugin: Some("grounding.core".into()),
        },
        SessionSubstate::Stopped,
        BackendSelection {
            capture_backend: "capture.window".into(),
            control_backend: "control.ble".into(),
        },
        None,
    )
    .unwrap_err();

    assert!(error.contains("does not match"));
}

#[test]
fn device_session_status_requires_operator_action_message_when_blocked() {
    let error = DeviceSessionStatus::new(
        DeviceSessionSummary {
            device_id: "device-3".into(),
            device_name: "Needs Help".into(),
            phase: SessionPhase::Degraded,
            plugin_health: PluginHealth::Degraded,
            capture_plugin: Some("capture.window".into()),
            control_plugin: Some("control.ble".into()),
            grounding_plugin: Some("grounding.core".into()),
        },
        SessionSubstate::OperatorActionRequired,
        BackendSelection {
            capture_backend: "capture.window".into(),
            control_backend: "control.ble".into(),
        },
        None,
    )
    .unwrap_err();

    assert!(error.contains("operator action"));
}

#[test]
fn device_session_status_rejects_operator_action_message_for_healthy_state() {
    let error = DeviceSessionStatus::new(
        DeviceSessionSummary {
            device_id: "device-4".into(),
            device_name: "Healthy".into(),
            phase: SessionPhase::Streaming,
            plugin_health: PluginHealth::Healthy,
            capture_plugin: Some("capture.window".into()),
            control_plugin: Some("control.ble".into()),
            grounding_plugin: Some("grounding.core".into()),
        },
        SessionSubstate::ControlReady,
        BackendSelection {
            capture_backend: "capture.window".into(),
            control_backend: "control.ble".into(),
        },
        Some("stale operator warning".into()),
    )
    .unwrap_err();

    assert!(error.contains("operator action"));
}
