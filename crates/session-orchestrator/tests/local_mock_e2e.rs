use ios_control_contracts::session::SessionPhase;
use ios_control_session_orchestrator::{PluginPaths, SessionOrchestrator, StartSessionRequest};

mod support;
use support::{build_plugins, plugin_path, workspace_root};

#[tokio::test]
async fn local_mock_e2e_builds_streaming_session() {
    let root = workspace_root();
    build_plugins(&root);

    let mut orchestrator = SessionOrchestrator::default();
    let state = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "device-e2e".into(),
            device_name: "Mock iPhone".into(),
            selected_source_id: Some("window-1".into()),
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                control: plugin_path(&root, "plugin-control-ble"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    // Keep this smoke test pinned to the developer flow documented in README.
    assert_eq!(state.summary.phase, SessionPhase::Streaming);
    assert_eq!(state.summary.capture_plugin.as_deref(), Some("capture.window"));
    assert_eq!(state.summary.control_plugin.as_deref(), Some("control.ble"));
    assert_eq!(
        state.summary.grounding_plugin.as_deref(),
        Some("grounding.core")
    );
    assert_eq!(state.selected_source_id.as_deref(), Some("window-1"));
    assert!(state.latest_frame.is_some());
    assert_eq!(
        state.diagnostics.grounding_summary.as_deref(),
        Some("selected pointer plan")
    );

    state.shutdown().await.unwrap();
}
