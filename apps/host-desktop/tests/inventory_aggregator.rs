use host_desktop::inventory::aggregator::aggregate_inventory;
use host_desktop::inventory::model::{
    CapabilityState, DeviceObservation, InventoryEvidenceSource, Sessionability,
};

#[test]
fn inventory_aggregator_merges_bluetooth_and_known_device_on_stable_id() {
    let snapshot = aggregate_inventory(vec![
        DeviceObservation {
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
        },
        DeviceObservation {
            provider: InventoryEvidenceSource::Known,
            stable_id: Some("bt:AA-BB".into()),
            known_device_id: Some("known-alice".into()),
            display_name: "Alice iPhone".into(),
            mirror_source_id: Some("window-helper-1".into()),
            live: false,
            capture_state: CapabilityState::Unavailable,
            preferred_control_state: CapabilityState::Unavailable,
            fallback_control_state: CapabilityState::Unavailable,
            reasons: vec!["known from history only".into()],
        },
    ]);

    assert_eq!(snapshot.devices.len(), 1);
    let device = &snapshot.devices[0];
    assert_eq!(device.known_device_id.as_deref(), Some("known-alice"));
    assert_eq!(device.stable_id.as_deref(), Some("bt:AA-BB"));
    assert!(device.live);
    assert!(device
        .evidence_sources
        .contains(&InventoryEvidenceSource::Bluetooth));
    assert!(device
        .evidence_sources
        .contains(&InventoryEvidenceSource::Known));
}

#[test]
fn inventory_aggregator_keeps_weak_name_only_matches_separate() {
    let snapshot = aggregate_inventory(vec![
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
        DeviceObservation {
            provider: InventoryEvidenceSource::Known,
            stable_id: None,
            known_device_id: Some("known-mirror".into()),
            display_name: "Operator Mirror".into(),
            mirror_source_id: None,
            live: false,
            capture_state: CapabilityState::Unavailable,
            preferred_control_state: CapabilityState::Unavailable,
            fallback_control_state: CapabilityState::Unavailable,
            reasons: vec!["known from history only".into()],
        },
    ]);

    assert_eq!(snapshot.devices.len(), 2);
}

#[test]
fn inventory_aggregator_marks_fallback_startable_when_capture_and_fallback_are_ready() {
    let snapshot = aggregate_inventory(vec![DeviceObservation {
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
    }]);

    assert_eq!(
        snapshot.devices[0].sessionability,
        Sessionability::StartableWithFallback
    );
}

#[test]
fn inventory_aggregator_keeps_known_only_rows_historical() {
    let snapshot = aggregate_inventory(vec![DeviceObservation {
        provider: InventoryEvidenceSource::Known,
        stable_id: None,
        known_device_id: Some("known-device".into()),
        display_name: "History iPhone".into(),
        mirror_source_id: None,
        live: false,
        capture_state: CapabilityState::Unavailable,
        preferred_control_state: CapabilityState::Unavailable,
        fallback_control_state: CapabilityState::Unavailable,
        reasons: vec!["known from history only".into()],
    }]);

    let device = &snapshot.devices[0];
    assert!(!device.live);
    assert_eq!(device.sessionability, Sessionability::Unknown);
    assert!(device.reasons.iter().any(|reason| reason.contains("history")));
}
