#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDetailViewModel {
    pub device_name: String,
    pub capture_source_labels: Vec<String>,
    pub control_checklist: Vec<String>,
}
