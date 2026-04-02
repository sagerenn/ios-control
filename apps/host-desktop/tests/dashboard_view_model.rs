use host_desktop::view_models::dashboard::DashboardViewModel;
use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{DeviceSessionSummary, SessionPhase};

#[test]
fn dashboard_view_model_counts_degraded_sessions() {
    let sessions = vec![
        DeviceSessionSummary {
            device_id: "a".into(),
            device_name: "iPhone".into(),
            phase: SessionPhase::Streaming,
            plugin_health: PluginHealth::Healthy,
            capture_plugin: Some("capture.mock".into()),
            control_plugin: Some("control.mock".into()),
            grounding_plugin: None,
        },
        DeviceSessionSummary {
            device_id: "b".into(),
            device_name: "iPad".into(),
            phase: SessionPhase::Degraded,
            plugin_health: PluginHealth::Degraded,
            capture_plugin: Some("capture.mock".into()),
            control_plugin: Some("control.mock".into()),
            grounding_plugin: None,
        },
    ];

    let view_model = DashboardViewModel::from_sessions(&sessions);

    assert_eq!(view_model.total_devices, 2);
    assert_eq!(view_model.degraded_devices, 1);
}
