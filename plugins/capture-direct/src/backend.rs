use async_trait::async_trait;
use ios_control_contracts::capture::{FrameHealth, SourceKind, VideoFrameDescriptor};

#[async_trait]
pub trait DirectReceiverBackend {
    async fn start_session(&self) -> anyhow::Result<VideoFrameDescriptor>;
}

pub fn first_frame(source_id: &str) -> VideoFrameDescriptor {
    VideoFrameDescriptor {
        source_id: source_id.into(),
        source_kind: SourceKind::DirectReceiver,
        width: 1179,
        height: 2556,
        rotation_degrees: 0,
        frame_index: 1,
        health: FrameHealth::Healthy,
    }
}
