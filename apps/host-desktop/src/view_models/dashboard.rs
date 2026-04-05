use ios_control_contracts::session::{DeviceSessionSummary, SessionPhase};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardViewModel {
    pub total_devices: usize,
    pub degraded_devices: usize,
}

impl DashboardViewModel {
    pub fn from_sessions(sessions: &[DeviceSessionSummary]) -> Self {
        let degraded_devices = sessions
            .iter()
            .filter(|session| session.phase == SessionPhase::Degraded)
            .count();

        Self {
            total_devices: sessions.len(),
            degraded_devices,
        }
    }

    pub fn from_inventory_rows(rows: usize, degraded_devices: usize) -> Self {
        Self {
            total_devices: rows,
            degraded_devices,
        }
    }
}
