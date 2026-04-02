use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    Window,
    DirectReceiver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameHealth {
    Healthy,
    Occluded,
    Stalled,
    Resized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoFrameDescriptor {
    pub source_id: String,
    pub source_kind: SourceKind,
    pub width: u32,
    pub height: u32,
    pub rotation_degrees: u16,
    pub frame_index: u64,
    pub health: FrameHealth,
}
