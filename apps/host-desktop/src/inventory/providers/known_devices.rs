use crate::inventory::model::{CapabilityState, DeviceObservation, InventoryEvidenceSource};
use crate::preferences::HostPreferences;

pub fn discover_known_devices(preferences: &HostPreferences) -> Vec<DeviceObservation> {
    preferences
        .known_devices
        .iter()
        .map(|known| DeviceObservation {
            provider: InventoryEvidenceSource::Known,
            stable_id: known.stable_id.clone(),
            known_device_id: Some(known.known_device_id.clone()),
            display_name: known.display_name.clone(),
            mirror_source_id: known.last_source_id.clone(),
            live: false,
            capture_state: CapabilityState::Unavailable,
            preferred_control_state: CapabilityState::Unavailable,
            fallback_control_state: CapabilityState::Unavailable,
            reasons: vec!["known from history only".into()],
        })
        .collect()
}
