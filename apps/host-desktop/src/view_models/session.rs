use crate::panels::device_detail::CaptureSourceOption;
use ios_control_contracts::capture::VideoFrameDescriptor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionViewModel {
    pub selected_source: Option<CaptureSourceOption>,
    pub latest_frame: Option<VideoFrameDescriptor>,
}
