use ios_control_session_orchestrator::PluginPaths;

use crate::inventory::model::{CapabilityState, DeviceObservation, InventoryEvidenceSource};
use crate::inventory::providers::{
    list_capture_sources, probe_capture_capability, probe_control_capability,
};
use crate::preferences::HostPreferences;

pub fn discover_mirror_devices(
    plugin_paths: &PluginPaths,
    preferences: &HostPreferences,
) -> Vec<DeviceObservation> {
    let _capture_capability = match probe_capture_capability(&plugin_paths.capture) {
        Ok(capability) if capability.available => capability,
        _ => return Vec::new(),
    };
    let sources = match list_capture_sources(&plugin_paths.capture) {
        Ok(sources) => sources,
        Err(_) => return Vec::new(),
    };
    let fallback_control_state =
        map_control_state(probe_control_capability(&plugin_paths.control_fallback).ok());

    sources
        .into_iter()
        .map(|source| {
            let known_device = preferences
                .known_devices
                .iter()
                .find(|known| known.last_source_id.as_deref() == Some(source.source_id.as_str()));
            let mut reasons = Vec::new();
            if let CapabilityState::Blocked(reason) = &fallback_control_state {
                reasons.push(reason.clone());
            }
            DeviceObservation {
                provider: InventoryEvidenceSource::Mirror,
                stable_id: known_device.and_then(|known| known.stable_id.clone()),
                known_device_id: known_device.map(|known| known.known_device_id.clone()),
                display_name: source.display_name,
                mirror_source_id: Some(source.source_id),
                live: true,
                capture_state: CapabilityState::Ready,
                preferred_control_state: CapabilityState::Unavailable,
                fallback_control_state: fallback_control_state.clone(),
                reasons,
            }
        })
        .collect()
}

fn map_control_state(
    capability: Option<ios_control_contracts::control::ControlCapability>,
) -> CapabilityState {
    match capability {
        Some(capability) if capability.supported => CapabilityState::Ready,
        Some(capability) => CapabilityState::Blocked(
            capability
                .reason
                .unwrap_or_else(|| "fallback control unavailable".into()),
        ),
        None => CapabilityState::Unavailable,
    }
}
