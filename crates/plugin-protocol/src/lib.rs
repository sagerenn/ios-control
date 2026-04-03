pub use ios_control_contracts::plugin::{PluginDescriptor, PluginKind};

use ios_control_contracts::capture::{VideoFrameDescriptor, VideoSource};
use ios_control_contracts::control::{
    ControlCapability, ControlSessionPhase, ControlSetupChecklist,
};
use ios_control_contracts::grounding::{GroundingPlan, GroundingRequest};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HostToPlugin {
    Handshake { protocol_version: u32 },
    ProbeControl,
    PrepareControl,
    ListCaptureSources,
    GetCaptureFrame { source_id: String },
    StartDirectCapture,
    PlanGrounding { request: GroundingRequest },
    Stop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginToHost {
    HandshakeAck {
        descriptor: PluginDescriptor,
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
    CaptureFrame {
        frame: VideoFrameDescriptor,
    },
    GroundingPlan {
        plan: GroundingPlan,
    },
    Ack,
    Error {
        message: String,
    },
}
