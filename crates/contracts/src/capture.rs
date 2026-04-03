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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureStreamPhase {
    Opening,
    Streaming,
    Closing,
    Closed,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoSource {
    pub source_id: String,
    pub display_name: String,
    pub kind: SourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureStreamDescriptor {
    pub source_id: String,
    pub source_kind: SourceKind,
    pub width: u32,
    pub height: u32,
    pub rotation_degrees: u16,
    pub slot_bytes: u32,
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
