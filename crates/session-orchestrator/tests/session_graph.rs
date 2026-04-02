use ios_control_contracts::session::SessionPhase;
use ios_control_session_orchestrator::{RequestedPlugins, SessionOrchestrator};

#[tokio::test]
async fn build_session_graph_uses_requested_plugins() {
    let orchestrator = SessionOrchestrator::default();
    let summary = orchestrator
        .start_session(
            "device-1",
            RequestedPlugins {
                capture: "capture.mock".into(),
                control: "control.mock".into(),
                grounding: Some("grounding.mock".into()),
            },
        )
        .await
        .unwrap();

    assert_eq!(summary.phase, SessionPhase::Connecting);
    assert_eq!(summary.capture_plugin.as_deref(), Some("capture.mock"));
    assert_eq!(summary.control_plugin.as_deref(), Some("control.mock"));
    assert_eq!(summary.grounding_plugin.as_deref(), Some("grounding.mock"));
}
