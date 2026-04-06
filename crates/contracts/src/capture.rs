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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioStreamPhase {
    Idle,
    Waiting,
    Streaming,
    Degraded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioRoute {
    None,
    LocalPlayback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioStreamStatus {
    pub phase: AudioStreamPhase,
    pub route: AudioRoute,
    pub active: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureStatus {
    pub video_phase: CaptureStreamPhase,
    pub video_health: FrameHealth,
    pub audio: AudioStreamStatus,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoSource {
    pub source_id: String,
    pub display_name: String,
    pub kind: SourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureCapability {
    pub available: bool,
    pub reason: Option<String>,
    pub backend_id: String,
    pub supports_input_bridge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureStreamDescriptor {
    pub source_id: String,
    pub source_kind: SourceKind,
    pub width: u32,
    pub height: u32,
    pub rotation_degrees: u16,
    pub slot_bytes: u32,
    pub slot_path: String,
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
