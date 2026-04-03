use crate::panels::device_detail::{CaptureSourceOption, ControlSetupChecklist};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDetailViewModel {
    pub device_name: String,
    pub capture_sources: Vec<CaptureSourceOption>,
    pub active_source_id: Option<String>,
    pub control_checklist: ControlSetupChecklist,
}
