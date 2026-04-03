use ios_control_contracts::session::DeviceSessionStatus;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HostRuntimeBridge {
    statuses: Vec<DeviceSessionStatus>,
    pending_start_device_id: Option<String>,
}

impl HostRuntimeBridge {
    pub fn replace_statuses(&mut self, statuses: Vec<DeviceSessionStatus>) {
        self.statuses = statuses;
    }

    pub fn statuses(&self) -> &[DeviceSessionStatus] {
        &self.statuses
    }

    pub fn queue_start(&mut self, device_id: String) {
        self.pending_start_device_id = Some(device_id);
    }

    pub fn take_pending_start(&mut self) -> Option<String> {
        self.pending_start_device_id.take()
    }
}
