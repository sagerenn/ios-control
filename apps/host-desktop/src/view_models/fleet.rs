use crate::inventory::model::{InventoryDevice, InventoryEvidenceSource, Sessionability};
use crate::inventory::model::CapabilityState;
use ios_control_contracts::session::DeviceSessionStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetRow {
    pub device_id: String,
    pub device_name: String,
    pub evidence_badges: Vec<String>,
    pub readiness_summary: String,
    pub start_enabled: bool,
    pub operator_action: Option<String>,
    pub active_session: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetViewModel {
    pub rows: Vec<FleetRow>,
}

impl FleetViewModel {
    pub fn for_launcher(
        devices: &[InventoryDevice],
        direct_receiver_available: bool,
        statuses: &[DeviceSessionStatus],
    ) -> Self {
        Self {
            rows: devices
                .iter()
                .filter(|device| {
                    device
                        .evidence_sources
                        .contains(&InventoryEvidenceSource::Bluetooth)
                })
                .map(|device| {
                    let status = statuses
                        .iter()
                        .find(|status| status.summary().device_id == device.inventory_id);
                    let start_enabled = direct_receiver_available && launcher_control_ready(device);
                    FleetRow {
                        device_id: device.inventory_id.clone(),
                        device_name: device.display_name.clone(),
                        evidence_badges: badges_for_device(device, status.is_some()),
                        readiness_summary: if start_enabled {
                            "Startable".into()
                        } else {
                            "Not Startable".into()
                        },
                        start_enabled,
                        operator_action: status
                            .and_then(|status| status.operator_action().map(str::to_string)),
                        active_session: status.is_some(),
                    }
                })
                .collect(),
        }
    }

    pub fn from_statuses(statuses: &[DeviceSessionStatus]) -> Self {
        Self {
            rows: statuses
                .iter()
                .map(|status| FleetRow {
                    device_id: status.summary().device_id.clone(),
                    device_name: status.summary().device_name.clone(),
                    evidence_badges: vec!["Active".into()],
                    readiness_summary: "Active session".into(),
                    start_enabled: false,
                    operator_action: status.operator_action().map(str::to_string),
                    active_session: true,
                })
                .collect(),
        }
    }

    pub fn from_inventory(
        devices: &[InventoryDevice],
        statuses: &[DeviceSessionStatus],
    ) -> Self {
        let mut rows: Vec<FleetRow> = devices
            .iter()
            .map(|device| {
                let status = statuses
                    .iter()
                    .find(|status| status.summary().device_id == device.inventory_id);
                FleetRow {
                    device_id: device.inventory_id.clone(),
                    device_name: device.display_name.clone(),
                    evidence_badges: badges_for_device(device, status.is_some()),
                    readiness_summary: readiness_summary(device, status.is_some()),
                    start_enabled: matches!(
                        device.sessionability,
                        Sessionability::StartableWithPreferredPath
                            | Sessionability::StartableWithFallback
                    ),
                    operator_action: status.and_then(|status| status.operator_action().map(str::to_string)),
                    active_session: status.is_some(),
                }
            })
            .collect();

        for status in statuses {
            if rows
                .iter()
                .any(|row| row.device_id == status.summary().device_id)
            {
                continue;
            }
            rows.push(FleetRow {
                device_id: status.summary().device_id.clone(),
                device_name: status.summary().device_name.clone(),
                evidence_badges: vec!["Active".into()],
                readiness_summary: "Active session".into(),
                start_enabled: false,
                operator_action: status.operator_action().map(str::to_string),
                active_session: true,
            });
        }

        Self { rows }
    }
}

fn launcher_control_ready(device: &InventoryDevice) -> bool {
    matches!(device.preferred_control_state, CapabilityState::Ready)
        || matches!(device.fallback_control_state, CapabilityState::Ready)
}

fn badges_for_device(device: &InventoryDevice, active_session: bool) -> Vec<String> {
    let mut badges = Vec::new();
    for source in &device.evidence_sources {
        badges.push(match source {
            InventoryEvidenceSource::Bluetooth => "Bluetooth",
            InventoryEvidenceSource::Mirror => "Mirror",
            InventoryEvidenceSource::Known => "Known",
        }
        .to_string());
    }
    if active_session {
        badges.push("Active".into());
    }
    badges
}

fn readiness_summary(device: &InventoryDevice, active_session: bool) -> String {
    if active_session {
        return "Active session".into();
    }
    match device.sessionability {
        Sessionability::StartableWithPreferredPath => "Startable".into(),
        Sessionability::StartableWithFallback => "Startable (fallback)".into(),
        Sessionability::NotStartable => "Not startable".into(),
        Sessionability::Unknown => "Historical".into(),
    }
}
