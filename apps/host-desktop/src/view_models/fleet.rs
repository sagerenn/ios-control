use ios_control_contracts::session::DeviceSessionStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetRow {
    pub device_id: String,
    pub device_name: String,
    pub capture_backend: String,
    pub control_backend: String,
    pub operator_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetViewModel {
    pub rows: Vec<FleetRow>,
}

impl FleetViewModel {
    pub fn from_statuses(statuses: &[DeviceSessionStatus]) -> Self {
        Self {
            rows: statuses
                .iter()
                .map(|status| FleetRow {
                    device_id: status.summary().device_id.clone(),
                    device_name: status.summary().device_name.clone(),
                    capture_backend: status.backends().capture_backend.clone(),
                    control_backend: status.backends().control_backend.clone(),
                    operator_action: status.operator_action().map(str::to_string),
                })
                .collect(),
        }
    }
}
