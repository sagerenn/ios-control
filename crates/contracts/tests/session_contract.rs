use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{DeviceSessionSummary, SessionPhase};

#[test]
fn device_session_summary_defaults_to_disconnected() {
    let summary = DeviceSessionSummary::new("device-1".into(), "iPhone 15".into());

    assert_eq!(summary.phase, SessionPhase::Disconnected);
    assert_eq!(summary.plugin_health, PluginHealth::Unknown);
    assert!(summary.capture_plugin.is_none());
    assert!(summary.control_plugin.is_none());
    assert!(summary.grounding_plugin.is_none());
}
