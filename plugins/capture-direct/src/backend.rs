use async_trait::async_trait;
use ios_control_contracts::capture::{FrameHealth, SourceKind, VideoFrameDescriptor};
use ios_control_frame_transport::FrameSlot;

pub const DIRECT_WIDTH: u32 = 1179;
pub const DIRECT_HEIGHT: u32 = 2556;
pub const DIRECT_SLOT_WIDTH: u32 = 3840;
pub const DIRECT_SLOT_HEIGHT: u32 = 2160;
pub const DIRECT_SLOT_BYTES: u32 = DIRECT_SLOT_WIDTH * DIRECT_SLOT_HEIGHT * 4;

#[async_trait]
pub trait DirectReceiverBackend {
    async fn start_session(&self) -> anyhow::Result<VideoFrameDescriptor>;
}

pub fn first_frame(source_id: &str) -> VideoFrameDescriptor {
    mock_frame(source_id, 1)
}

pub fn mock_frame(source_id: &str, frame_index: u64) -> VideoFrameDescriptor {
    VideoFrameDescriptor {
        source_id: source_id.into(),
        source_kind: SourceKind::DirectReceiver,
        width: DIRECT_WIDTH,
        height: DIRECT_HEIGHT,
        rotation_degrees: 0,
        frame_index,
        health: FrameHealth::Healthy,
    }
}

pub fn allocate_mock_slot() -> anyhow::Result<FrameSlot> {
    FrameSlot::new(DIRECT_SLOT_BYTES as usize)
}

pub fn mock_frame_bytes() -> Vec<u8> {
    vec![64_u8; (DIRECT_WIDTH as usize) * (DIRECT_HEIGHT as usize) * 4]
}
