use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::SessionPhase;
use ios_control_session_orchestrator::{PluginPaths, SessionOrchestrator, StartSessionRequest};

mod support;
use support::{
    build_plugins, plugin_path, prepare_window_runtime_env, runtime_env_lock, workspace_root,
};

#[tokio::test]
async fn start_session_collects_mock_plugin_state() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _display_guard = prepare_window_runtime_env(&root);

    let mut orchestrator = SessionOrchestrator::default();
    let state = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "device-1".into(),
            device_name: "Mock iPhone".into(),
            selected_source_id: Some("window-helper-1".into()),
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                control_ble: plugin_path(&root, "plugin-control-ble"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    assert_eq!(state.summary.device_id, "device-1");
    assert_eq!(state.summary.device_name, "Mock iPhone");
    assert_eq!(state.summary.phase, SessionPhase::Degraded);
    assert_eq!(state.summary.plugin_health, PluginHealth::Degraded);
    assert_eq!(
        state.summary.capture_plugin.as_deref(),
        Some("capture.window")
    );
    assert_eq!(state.summary.control_plugin.as_deref(), Some("control.ble"));
    assert_eq!(
        state.summary.grounding_plugin.as_deref(),
        Some("grounding.core")
    );
    assert_eq!(state.selected_source_id.as_deref(), Some("window-helper-1"));

    assert_eq!(state.capture_sources.len(), 1);
    assert_eq!(state.capture_sources[0].source_id, "window-helper-1");
    assert!(state.capture_stream.is_some());
    assert_eq!(state.latest_frame.as_ref().unwrap().source_id, "window-helper-1");
    assert!(state
        .diagnostics
        .grounding_summary
        .as_deref()
        .unwrap()
        .contains("selected"));
    assert!(state.execution_result.is_some());
    let execution = state.execution_result.as_ref().unwrap();
    assert!(!execution.applied);
    assert!(!execution.observed_change);
    assert_eq!(execution.phase, ios_control_contracts::control::ExecutionPhase::Failed);
    assert_eq!(execution.attempts, 1);
    assert_eq!(
        execution.grounding_failure,
        None
    );
    assert!(execution.summary.contains("failure:"));
    assert!(execution.failure_reason.is_some());

    let control_capability = orchestrator.capabilities.get("control.ble").unwrap();
    if control_capability.supported {
        assert!(state.control_checklist.items.len() >= 2);
        assert!(state
            .control_checklist
            .items
            .iter()
            .any(|item| item.contains("Enable Bluetooth")));
        assert!(state.diagnostics.control_summary.contains("supported"));
        assert_eq!(state.summary.plugin_health, PluginHealth::Degraded);
        assert_eq!(control_capability.reason, None);
    } else {
        assert!(!state.control_checklist.items.is_empty());
        assert!(state.diagnostics.control_summary.contains("unsupported"));
        assert!(control_capability.reason.is_some());
        assert_eq!(state.summary.plugin_health, PluginHealth::Degraded);
    }

    let device = orchestrator.devices.get("device-1").unwrap();
    assert_eq!(device.device_name, "Mock iPhone");
    assert_eq!(device.preferred_capture_plugin, "capture.window");
    assert_eq!(device.preferred_control_plugin, "control.ble");
    assert_eq!(
        device.preferred_grounding_plugin.as_deref(),
        Some("grounding.core")
    );
    assert_eq!(device.last_source_id.as_deref(), Some("window-helper-1"));

    let telemetry = orchestrator.telemetry.for_session("device-1");
    assert!(telemetry
        .iter()
        .any(|event| event.message == "session started"));
    assert!(telemetry
        .iter()
        .any(|event| event.message.contains("capture source")));
    assert!(telemetry
        .iter()
        .any(|event| event.message.contains("grounding planned")));

    state.shutdown().await.unwrap();
}

#[tokio::test]
async fn start_session_failure_does_not_persist_partial_state() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _display_guard = prepare_window_runtime_env(&root);

    let mut orchestrator = SessionOrchestrator::default();
    let error = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "device-2".into(),
            device_name: "Broken Mock iPhone".into(),
            selected_source_id: Some("missing-source".into()),
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                control_ble: plugin_path(&root, "plugin-control-ble"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("requested capture source `missing-source` is unavailable"));
    assert!(orchestrator.capabilities.entries().is_empty());
    assert!(orchestrator.devices.entries().is_empty());
    assert!(orchestrator.telemetry.events().is_empty());
}

#[tokio::test]
async fn start_session_opens_capture_stream_and_refreshes_frames() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _display_guard = prepare_window_runtime_env(&root);

    let mut orchestrator = SessionOrchestrator::default();
    let mut state = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "device-refresh".into(),
            device_name: "Refresh Mock iPhone".into(),
            selected_source_id: Some("window-helper-1".into()),
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                control_ble: plugin_path(&root, "plugin-control-ble"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    assert!(state.capture_stream.is_some());
    let previous = state.latest_frame.as_ref().unwrap().frame_index;
    let refreshed = state.refresh_capture_frame().await.unwrap();
    assert!(refreshed.frame_index > previous);
    assert_eq!(
        state.capture_stream.as_ref().unwrap().source_id,
        "window-helper-1"
    );

    state.shutdown().await.unwrap();
}
