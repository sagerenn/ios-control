use crate::panels::device_detail::{CaptureSourceOption, ControlSetupChecklist};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDetailViewModel {
    pub device_name: String,
    pub capture_sources: Vec<CaptureSourceOption>,
    pub active_source_id: Option<String>,
    pub control_checklist: ControlSetupChecklist,
}

impl DeviceDetailViewModel {
    pub fn capture_source(&self, source_id: &str) -> Option<CaptureSourceOption> {
        self.capture_sources
            .iter()
            .find(|source| source.source_id == source_id)
            .cloned()
    }
}
