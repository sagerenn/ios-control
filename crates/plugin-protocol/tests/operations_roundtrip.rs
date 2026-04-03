use ios_control_contracts::grounding::{GroundingPlan, GroundingRequest, PlanKind, TargetInput};
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};

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
