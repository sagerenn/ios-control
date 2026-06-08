use std::collections::BTreeSet;

use crate::inventory::model::{
    CapabilityState, DeviceObservation, InventoryDevice, InventorySnapshot, Sessionability,
};

pub fn aggregate_inventory(observations: Vec<DeviceObservation>) -> InventorySnapshot {
    let mut devices: Vec<InventoryDevice> = Vec::new();

    for observation in observations {
        if let Some(existing) = devices
            .iter_mut()
            .find(|device| should_merge(device, &observation))
        {
            merge_observation(existing, observation);
        } else {
            devices.push(device_from_observation(observation));
        }
    }

    for device in &mut devices {
        device.sessionability = compose_sessionability(
            &device.capture_state,
            &device.preferred_control_state,
            &device.fallback_control_state,
            device.live,
        );
    }

    InventorySnapshot { devices }
}

fn should_merge(device: &InventoryDevice, observation: &DeviceObservation) -> bool {
    if let (Some(existing), Some(incoming)) = (&device.stable_id, &observation.stable_id) {
        if existing == incoming {
            return true;
        }
    }

    if let (Some(existing), Some(incoming)) =
        (&device.known_device_id, &observation.known_device_id)
    {
        if existing == incoming && (device.live || observation.live) {
            return true;
        }
    }

    if let (
        Some(existing_known),
        Some(incoming_known),
        Some(existing_source),
        Some(incoming_source),
    ) = (
        &device.known_device_id,
        &observation.known_device_id,
        &device.mirror_source_id,
        &observation.mirror_source_id,
    ) {
        return existing_known == incoming_known && existing_source == incoming_source;
    }

    false
}

fn merge_observation(device: &mut InventoryDevice, observation: DeviceObservation) {
    if device.stable_id.is_none() {
        device.stable_id = observation.stable_id.clone();
    }
    if device.known_device_id.is_none() {
        device.known_device_id = observation.known_device_id.clone();
    }
    if device.mirror_source_id.is_none() {
        device.mirror_source_id = observation.mirror_source_id.clone();
    }
    if device.display_name.is_empty() {
        device.display_name = observation.display_name.clone();
    }
    device.live |= observation.live;
    push_evidence_source(&mut device.evidence_sources, observation.provider);
    device.capture_state = best_state(&device.capture_state, &observation.capture_state).clone();
    device.preferred_control_state = best_state(
        &device.preferred_control_state,
        &observation.preferred_control_state,
    )
    .clone();
    device.fallback_control_state = best_state(
        &device.fallback_control_state,
        &observation.fallback_control_state,
    )
    .clone();
    merge_reasons(&mut device.reasons, &observation.reasons);
}

fn device_from_observation(observation: DeviceObservation) -> InventoryDevice {
    let inventory_id = observation
        .known_device_id
        .clone()
        .or_else(|| observation.stable_id.clone())
        .or_else(|| observation.mirror_source_id.clone())
        .unwrap_or_else(|| format!("inventory:{}", observation.display_name));
    let sessionability = compose_sessionability(
        &observation.capture_state,
        &observation.preferred_control_state,
        &observation.fallback_control_state,
        observation.live,
    );

    InventoryDevice {
        inventory_id,
        display_name: observation.display_name,
        stable_id: observation.stable_id,
        known_device_id: observation.known_device_id,
        mirror_source_id: observation.mirror_source_id,
        live: observation.live,
        evidence_sources: vec![observation.provider],
        capture_state: observation.capture_state,
        preferred_control_state: observation.preferred_control_state,
        fallback_control_state: observation.fallback_control_state,
        sessionability,
        reasons: observation.reasons,
    }
}

fn compose_sessionability(
    capture_state: &CapabilityState,
    preferred_control_state: &CapabilityState,
    fallback_control_state: &CapabilityState,
    live: bool,
) -> Sessionability {
    if matches!(capture_state, CapabilityState::Ready)
        && matches!(preferred_control_state, CapabilityState::Ready)
    {
        return Sessionability::StartableWithPreferredPath;
    }
    if matches!(capture_state, CapabilityState::Ready)
        && matches!(fallback_control_state, CapabilityState::Ready)
    {
        return Sessionability::StartableWithFallback;
    }
    if live {
        Sessionability::NotStartable
    } else {
        Sessionability::Unknown
    }
}

fn best_state<'a>(
    current: &'a CapabilityState,
    incoming: &'a CapabilityState,
) -> &'a CapabilityState {
    if score_state(incoming) > score_state(current) {
        incoming
    } else {
        current
    }
}

fn score_state(state: &CapabilityState) -> u8 {
    match state {
        CapabilityState::Ready => 3,
        CapabilityState::Discovered => 2,
        CapabilityState::Blocked(_) => 1,
        CapabilityState::Unavailable => 0,
    }
}

fn push_evidence_source(
    sources: &mut Vec<crate::inventory::model::InventoryEvidenceSource>,
    source: crate::inventory::model::InventoryEvidenceSource,
) {
    let mut set: BTreeSet<_> = sources.iter().copied().collect();
    set.insert(source);
    *sources = set.into_iter().collect();
}

fn merge_reasons(existing: &mut Vec<String>, incoming: &[String]) {
    let mut set: BTreeSet<String> = existing.iter().cloned().collect();
    set.extend(incoming.iter().cloned());
    *existing = set.into_iter().collect();
}
