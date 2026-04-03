use ios_control_contracts::session::DeviceSessionStatus;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HostRuntimeBridge {
    statuses: Vec<DeviceSessionStatus>,
}

impl HostRuntimeBridge {
    pub fn replace_statuses(&mut self, statuses: Vec<DeviceSessionStatus>) {
        self.statuses = statuses;
    }

    pub fn statuses(&self) -> &[DeviceSessionStatus] {
        &self.statuses
    }
}
