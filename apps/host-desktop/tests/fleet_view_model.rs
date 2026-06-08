use host_desktop::inventory::aggregator::aggregate_inventory;
use host_desktop::inventory::model::{CapabilityState, DeviceObservation, InventoryEvidenceSource};
use host_desktop::view_models::fleet::FleetViewModel;
use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{
    BackendSelection, DeviceSessionStatus, DeviceSessionSummary, SessionPhase, SessionSubstate,
};

#[test]
fn fleet_view_model_launcher_filters_to_bluetooth_rows() {
    let inventory = aggregate_inventory(vec![
        DeviceObservation {
            provider: InventoryEvidenceSource::Bluetooth,
            stable_id: Some("bt:AA-BB".into()),
            known_device_id: None,
            display_name: "Alice iPhone".into(),
            mirror_source_id: None,
            live: true,
            capture_state: CapabilityState::Unavailable,
            preferred_control_state: CapabilityState::Ready,
            fallback_control_state: CapabilityState::Unavailable,
            reasons: vec!["paired over bluetooth".into()],
        },
        DeviceObservation {
            provider: InventoryEvidenceSource::Mirror,
            stable_id: None,
            known_device_id: None,
            display_name: "Operator Mirror".into(),
            mirror_source_id: Some("window-helper-1".into()),
            live: true,
            capture_state: CapabilityState::Ready,
            preferred_control_state: CapabilityState::Unavailable,
            fallback_control_state: CapabilityState::Ready,
            reasons: vec![],
        },
    ]);

    let fleet = FleetViewModel::for_launcher(&inventory.devices, true, &[]);
    assert_eq!(fleet.rows.len(), 1);
    assert_eq!(fleet.rows[0].device_name, "Alice iPhone");
    assert_eq!(fleet.rows[0].readiness_summary, "Startable");
    assert!(fleet.rows[0].start_enabled);
}

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
    assert_eq!(
        fleet.rows[1].operator_action.as_deref(),
        Some("reconnect mirror helper")
    );
}

#[test]
fn fleet_view_model_surfaces_inventory_badges_and_readiness() {
    let inventory = aggregate_inventory(vec![DeviceObservation {
        provider: InventoryEvidenceSource::Bluetooth,
        stable_id: Some("bt:AA-BB".into()),
        known_device_id: None,
        display_name: "Alice iPhone".into(),
        mirror_source_id: None,
        live: true,
        capture_state: CapabilityState::Unavailable,
        preferred_control_state: CapabilityState::Discovered,
        fallback_control_state: CapabilityState::Unavailable,
        reasons: vec!["paired over bluetooth".into()],
    }]);

    let fleet = FleetViewModel::from_inventory(&inventory.devices, &[]);
    assert_eq!(fleet.rows.len(), 1);
    assert!(fleet.rows[0]
        .evidence_badges
        .iter()
        .any(|badge| badge == "Bluetooth"));
    assert_eq!(fleet.rows[0].readiness_summary, "Not startable");
    assert!(!fleet.rows[0].start_enabled);
}
