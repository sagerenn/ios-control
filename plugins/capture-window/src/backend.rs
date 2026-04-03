use async_trait::async_trait;
use ios_control_contracts::capture::{FrameHealth, SourceKind, VideoFrameDescriptor};
use ios_control_frame_transport::FrameSlot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSource {
    pub source_id: String,
    pub display_name: String,
}

#[async_trait]
pub trait WindowCaptureBackend {
    async fn list_sources(&mut self) -> anyhow::Result<Vec<WindowSource>>;
    async fn next_frame(&mut self, source_id: &str) -> anyhow::Result<VideoFrameDescriptor>;
}

pub fn mock_frame(source_id: &str, frame_index: u64) -> VideoFrameDescriptor {
    VideoFrameDescriptor {
        source_id: source_id.into(),
        source_kind: SourceKind::Window,
        width: 1280,
        height: 720,
        rotation_degrees: 0,
        frame_index,
        health: FrameHealth::Healthy,
    }
}

pub fn allocate_mock_slot() -> anyhow::Result<FrameSlot> {
    FrameSlot::new(mock_frame_bytes().len())
}

pub fn mock_frame_bytes() -> Vec<u8> {
    vec![128_u8; 1280 * 720 * 4]
}
