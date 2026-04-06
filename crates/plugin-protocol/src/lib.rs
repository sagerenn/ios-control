pub use ios_control_contracts::plugin::{PluginDescriptor, PluginKind};

use ios_control_contracts::capture::{
    CaptureCapability, CaptureStatus, CaptureStreamDescriptor, VideoFrameDescriptor, VideoSource,
};
use ios_control_contracts::control::{
    ControlCapability, ControlSessionPhase, ControlSetupChecklist, ExecutionSummary,
};
use ios_control_contracts::grounding::{GroundingPlan, GroundingRequest};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HostToPlugin {
    Handshake { protocol_version: u32 },
    ProbeCapture,
    ProbeControl,
    PrepareControl,
    ListCaptureSources,
    OpenCaptureStream { source_id: String },
    ReadCaptureFrame,
    CloseCaptureStream,
    GetCaptureFrame { source_id: String },
    GetCaptureStatus,
    StartDirectCapture,
    PlanGrounding { request: GroundingRequest },
    ExecutePlan { plan: GroundingPlan },
    Stop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginToHost {
    HandshakeAck {
        descriptor: PluginDescriptor,
    },
    CaptureCapability {
        capability: CaptureCapability,
    },
    ControlCapability {
        capability: ControlCapability,
    },
    ControlSession {
        phase: ControlSessionPhase,
        checklist: ControlSetupChecklist,
    },
    CaptureSources {
        sources: Vec<VideoSource>,
    },
    CaptureStreamOpened {
        stream: CaptureStreamDescriptor,
    },
    CaptureFrame {
        frame: VideoFrameDescriptor,
    },
    CaptureStatus {
        status: CaptureStatus,
    },
    ExecutionSummary {
        summary: ExecutionSummary,
    },
    GroundingPlan {
        plan: GroundingPlan,
    },
    Ack,
    Error {
        message: String,
    },
}
