use host_desktop::view_models::fleet::FleetViewModel;
use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{
    BackendSelection, DeviceSessionStatus, DeviceSessionSummary, SessionPhase, SessionSubstate,
};

#[test]
fn fleet_view_model_preserves_operator_actions_per_device() {
    let statuses = vec![
        DeviceSessionStatus::new(
            DeviceSessionSummary {
                device_id: "device-1".into(),
                device_name: "Alpha".into(),
                phase: SessionPhase::Streaming,
                plugin_health: PluginHealth::Healthy,
                capture_plugin: Some("capture.window.helper".into()),
                control_plugin: Some("control.ble".into()),
                grounding_plugin: Some("grounding.core".into()),
            },
            SessionSubstate::ControlReady,
            BackendSelection {
                capture_backend: "capture.window.helper".into(),
                control_backend: "control.ble".into(),
            },
            None,
        )
        .expect("valid streaming status"),
        DeviceSessionStatus::new(
            DeviceSessionSummary {
                device_id: "device-2".into(),
                device_name: "Beta".into(),
                phase: SessionPhase::Degraded,
                plugin_health: PluginHealth::Degraded,
                capture_plugin: Some("capture.window.helper".into()),
                control_plugin: Some("control.window-bridge".into()),
                grounding_plugin: Some("grounding.core".into()),
            },
            SessionSubstate::OperatorActionRequired,
            BackendSelection {
                capture_backend: "capture.window.helper".into(),
                control_backend: "control.window-bridge".into(),
            },
            Some("reconnect mirror helper".into()),
        )
        .expect("valid degraded status with operator action"),
    ];

    let fleet = FleetViewModel::from_statuses(&statuses);
    assert_eq!(fleet.rows.len(), 2);
    assert_eq!(fleet.rows[1].operator_action.as_deref(), Some("reconnect mirror helper"));
}
