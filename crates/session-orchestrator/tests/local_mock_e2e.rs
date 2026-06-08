use ios_control_contracts::session::SessionPhase;
use ios_control_session_orchestrator::{
    CaptureBackend, PluginPaths, SessionOrchestrator, StartSessionRequest,
};

mod support;
use support::{
    build_plugins, plugin_path, prepare_window_runtime_env, runtime_env_lock, workspace_root,
};

#[tokio::test]
async fn local_mock_e2e_builds_streaming_session() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _display_guard = prepare_window_runtime_env(&root);

    let mut orchestrator = SessionOrchestrator::default();
    let state = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "device-e2e".into(),
            device_name: "Mock iPhone".into(),
            selected_source_id: Some("window-helper-1".into()),
            capture_backend: CaptureBackend::Window,
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                capture_direct: plugin_path(&root, "plugin-capture-direct"),
                capture_direct_runtime_root: None,
                control_ble: plugin_path(&root, "plugin-control-ble"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    // Keep this smoke test pinned to the developer flow documented in README,
    // and to the real-device acceptance matrix for current validation status.
    assert_eq!(state.summary.phase, SessionPhase::Streaming);
    assert_eq!(
        state.summary.capture_plugin.as_deref(),
        Some("capture.window")
    );
    assert_eq!(
        state.summary.control_plugin.as_deref(),
        Some("control.window-bridge")
    );
    assert_eq!(
        state.summary.grounding_plugin.as_deref(),
        Some("grounding.core")
    );
    assert_eq!(state.selected_source_id.as_deref(), Some("window-helper-1"));
    assert!(state.capture_stream.is_some());
    assert!(state.latest_frame.is_some());
    assert_eq!(
        state.diagnostics.grounding_summary.as_deref(),
        Some("selected pointer plan")
    );
    assert!(state.execution_result.is_some());
    assert!(state.execution_result.as_ref().unwrap().applied);
    assert!(state.execution_result.as_ref().unwrap().observed_change);

    state.shutdown().await.unwrap();
}
