use ios_control_contracts::capture::{
    CaptureCapability, CaptureStreamDescriptor, VideoFrameDescriptor,
};
use ios_control_contracts::control::{ExecutionPhase, ExecutionSummary};
use ios_control_contracts::grounding::{GroundingPlan, GroundingRequest, PlanKind, TargetInput};
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};

#[test]
fn host_to_plugin_roundtrips_operational_messages() {
    let request = HostToPlugin::PlanGrounding {
        request: GroundingRequest {
            target: TargetInput {
                semantic_label: Some("Settings".into()),
                visual_region: Some((20, 20, 120, 44)),
                confidence: 0.94,
            },
            device_size: (1179, 2556),
            pointer_estimate: (60.0, 40.0),
            uncertainty_radius: 8.0,
            focus_confidence: 0.75,
            keyboard_preferred: false,
        },
    };

    let json = serde_json::to_string(&request).unwrap();
    let decoded: HostToPlugin = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, request);

    let response = PluginToHost::GroundingPlan {
        plan: GroundingPlan {
            kind: PlanKind::Pointer,
            failure: None,
            summary: "tap target".into(),
        },
    };

    let response_json = serde_json::to_string(&response).unwrap();
    let decoded_response: PluginToHost = serde_json::from_str(&response_json).unwrap();
    assert_eq!(decoded_response, response);
}

#[test]
fn capture_stream_messages_roundtrip() {
    let request = HostToPlugin::OpenCaptureStream {
        source_id: "window-1".into(),
    };
    let json = serde_json::to_string(&request).unwrap();
    let decoded: HostToPlugin = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, request);

    let response = PluginToHost::CaptureStreamOpened {
        stream: CaptureStreamDescriptor {
            source_id: "window-1".into(),
            source_kind: ios_control_contracts::capture::SourceKind::Window,
            width: 1280,
            height: 720,
            rotation_degrees: 0,
            slot_bytes: 1280 * 720 * 4,
            slot_path: "/tmp/mock-frame-slot".into(),
        },
    };
    let response_json = serde_json::to_string(&response).unwrap();
    let decoded_response: PluginToHost = serde_json::from_str(&response_json).unwrap();
    assert_eq!(decoded_response, response);
}

#[test]
fn execution_messages_roundtrip() {
    let plan = GroundingPlan {
        kind: PlanKind::Keyboard,
        failure: None,
        summary: "selected keyboard plan".into(),
    };

    let request = HostToPlugin::ExecutePlan { plan: plan.clone() };
    let json = serde_json::to_string(&request).unwrap();
    let decoded: HostToPlugin = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, request);

    let response = PluginToHost::ExecutionSummary {
        summary: ExecutionSummary {
            summary: "executed keyboard plan".into(),
            phase: ExecutionPhase::Succeeded,
            failure_reason: None,
        },
    };
    let response_json = serde_json::to_string(&response).unwrap();
    let decoded_response: PluginToHost = serde_json::from_str(&response_json).unwrap();
    assert_eq!(decoded_response, response);
}

#[test]
fn incremental_stream_messages_roundtrip() {
    let request = HostToPlugin::ReadCaptureFrame;
    let json = serde_json::to_string(&request).unwrap();
    let decoded: HostToPlugin = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, request);

    let close_request = HostToPlugin::CloseCaptureStream;
    let close_json = serde_json::to_string(&close_request).unwrap();
    let close_decoded: HostToPlugin = serde_json::from_str(&close_json).unwrap();
    assert_eq!(close_decoded, close_request);

    let response = PluginToHost::CaptureFrame {
        frame: VideoFrameDescriptor {
            source_id: "window-1".into(),
            source_kind: ios_control_contracts::capture::SourceKind::Window,
            width: 1280,
            height: 720,
            rotation_degrees: 0,
            frame_index: 7,
            health: ios_control_contracts::capture::FrameHealth::Healthy,
        },
    };
    let response_json = serde_json::to_string(&response).unwrap();
    let decoded_response: PluginToHost = serde_json::from_str(&response_json).unwrap();
    assert_eq!(decoded_response, response);
}

#[test]
fn capture_probe_messages_roundtrip() {
    let request = HostToPlugin::ProbeCapture;
    let json = serde_json::to_string(&request).unwrap();
    let decoded: HostToPlugin = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, request);

    let response = PluginToHost::CaptureCapability {
        capability: CaptureCapability {
            available: true,
            reason: None,
            backend_id: "capture.window".into(),
            supports_input_bridge: true,
        },
    };
    let response_json = serde_json::to_string(&response).unwrap();
    let decoded_response: PluginToHost = serde_json::from_str(&response_json).unwrap();
    assert_eq!(decoded_response, response);
}

#[test]
fn handshake_roundtrips_plugin_descriptor() {
    let response = PluginToHost::HandshakeAck {
        descriptor: PluginDescriptor {
            plugin_id: "mock.device".into(),
            protocol_version: 3,
            kind: PluginKind::Control,
            display_name: "Mock Device".into(),
        },
    };

    let json = serde_json::to_string(&response).unwrap();
    let decoded: PluginToHost = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, response);
}
