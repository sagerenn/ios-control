use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::SessionPhase;
use ios_control_session_orchestrator::{
    CaptureBackend, PluginPaths, SessionOrchestrator, SessionSupervisor, StartSessionRequest,
};

mod support;
use support::{
    EnvVarGuard, build_plugins, plugin_path, prepare_window_runtime_env, runtime_env_lock,
    workspace_root, write_ble_helper, write_direct_helper,
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
            capture_backend: CaptureBackend::Window,
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                capture_direct: plugin_path(&root, "plugin-capture-direct"),
                control_ble: plugin_path(&root, "plugin-control-ble"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    assert_eq!(state.summary.device_id, "device-1");
    assert_eq!(state.summary.device_name, "Mock iPhone");
    assert_eq!(state.summary.phase, SessionPhase::Streaming);
    assert_eq!(state.summary.plugin_health, PluginHealth::Healthy);
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

    assert_eq!(state.capture_sources.len(), 1);
    assert_eq!(state.capture_sources[0].source_id, "window-helper-1");
    assert!(state.capture_stream.is_some());
    assert_eq!(
        state.latest_frame.as_ref().unwrap().source_id,
        "window-helper-1"
    );
    assert!(state
        .diagnostics
        .grounding_summary
        .as_deref()
        .unwrap()
        .contains("selected"));
    assert!(state.execution_result.is_some());
    let execution = state.execution_result.as_ref().unwrap();
    assert!(execution.applied);
    assert!(execution.observed_change);
    assert_eq!(
        execution.phase,
        ios_control_contracts::control::ExecutionPhase::Succeeded
    );
    assert_eq!(execution.attempts, 1);
    assert_eq!(execution.grounding_failure, None);
    assert!(execution.summary.contains("screen_changed=true"));
    assert!(execution.summary.contains("observed-change success"));
    assert_eq!(execution.failure_reason, None);

    let control_capability = orchestrator
        .capabilities
        .get("control.window-bridge")
        .unwrap();
    assert!(control_capability.supported);
    assert_eq!(control_capability.reason, None);
    assert!(!state.control_checklist.items.is_empty());
    assert!(state
        .control_checklist
        .items
        .iter()
        .all(|item| !item.trim().is_empty()));
    assert!(state.diagnostics.control_summary.contains("supported"));
    assert_eq!(state.summary.plugin_health, PluginHealth::Healthy);

    let device = orchestrator.devices.get("device-1").unwrap();
    assert_eq!(device.device_name, "Mock iPhone");
    assert_eq!(device.preferred_capture_plugin, "capture.window");
    assert_eq!(device.preferred_control_plugin, "control.window-bridge");
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
async fn execution_result_marks_observed_change_as_applied() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _display_guard = prepare_window_runtime_env(&root);
    let helper = write_ble_helper(
        r#"{"supported":true,"supports_prepare":true,"supports_execute":true,"supports_status":true,"supports_stop":true,"supports_forget_bond":true}"#,
        r#"{"phase":"Connected","checklist":["Pair the device"],"notes":[]}"#,
        r#"{"phase":"Succeeded","summary":"tap's applied","observed_change":true}"#,
    );
    let _helper_guard = EnvVarGuard::set("IOS_CONTROL_BLE_HELPER", &helper);

    let mut orchestrator = SessionOrchestrator::default();
    let state = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "device-observed".into(),
            device_name: "Observed Change iPhone".into(),
            selected_source_id: Some("window-helper-1".into()),
            capture_backend: CaptureBackend::Window,
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                capture_direct: plugin_path(&root, "plugin-capture-direct"),
                control_ble: plugin_path(&root, "plugin-control-ble"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    assert_eq!(state.summary.control_plugin.as_deref(), Some("control.ble"));
    let execution = state.execution_result.as_ref().unwrap();
    assert!(execution.applied);
    assert!(execution.observed_change);
    assert!(execution.summary.contains("tap's applied"));
    assert!(execution.summary.contains("observed-change success"));

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
            capture_backend: CaptureBackend::Window,
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                capture_direct: plugin_path(&root, "plugin-capture-direct"),
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
            capture_backend: CaptureBackend::Window,
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                capture_direct: plugin_path(&root, "plugin-capture-direct"),
                control_ble: plugin_path(&root, "plugin-control-ble"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    assert!(state.capture_stream.is_some());
    let previous = state.latest_frame.as_ref().unwrap().frame_index;
    let refreshed = state.refresh_capture_frame().await.unwrap().unwrap();
    assert!(refreshed.frame_index > previous);
    assert_eq!(
        state.capture_stream.as_ref().unwrap().source_id,
        "window-helper-1"
    );

    state.shutdown().await.unwrap();
}

#[tokio::test]
async fn supervisor_refresh_session_updates_active_latest_frame() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _display_guard = prepare_window_runtime_env(&root);

    let mut supervisor = SessionSupervisor::default();
    supervisor
        .start_or_replace_session(StartSessionRequest {
            device_id: "device-supervisor-refresh".into(),
            device_name: "Refresh Mock iPhone".into(),
            selected_source_id: Some("window-helper-1".into()),
            capture_backend: CaptureBackend::Window,
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                capture_direct: plugin_path(&root, "plugin-capture-direct"),
                control_ble: plugin_path(&root, "plugin-control-ble"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    let first = supervisor
        .active_sessions()
        .get("device-supervisor-refresh")
        .and_then(|session| session.latest_frame.as_ref())
        .unwrap()
        .frame_index;

    supervisor
        .refresh_session("device-supervisor-refresh")
        .await
        .unwrap();

    let refreshed = supervisor
        .active_sessions()
        .get("device-supervisor-refresh")
        .and_then(|session| session.latest_frame.as_ref())
        .unwrap()
        .frame_index;

    assert!(refreshed > first);
    assert!(supervisor
        .session_statuses()
        .contains_key("device-supervisor-refresh"));

    supervisor
        .stop_session("device-supervisor-refresh")
        .await
        .unwrap();
}

#[tokio::test]
async fn start_session_with_direct_backend_uses_capture_direct_plugin() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);

    let mut orchestrator = SessionOrchestrator::default();
    let state = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "direct-session".into(),
            device_name: "Direct Receiver".into(),
            selected_source_id: Some("direct-1".into()),
            capture_backend: CaptureBackend::Direct,
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                capture_direct: plugin_path(&root, "plugin-capture-direct"),
                control_ble: plugin_path(&root, "plugin-control-ble"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    assert_eq!(state.summary.capture_plugin.as_deref(), Some("capture.direct"));
    assert_eq!(state.selected_source_id.as_deref(), Some("direct-1"));
    assert_eq!(state.latest_frame.as_ref().map(|frame| frame.source_id.as_str()), Some("direct-1"));

    state.shutdown().await.unwrap();
}

#[tokio::test]
async fn start_session_with_direct_backend_can_wait_for_first_frame() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let direct_plugin = plugin_path(&root, "plugin-capture-direct");
    let helper = write_direct_helper(&format!(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  echo '{{"available":true,"supports_input_bridge":false}}'
  exit 0
fi
if [ "$1" = "stream" ]; then
  sleep 3
  exit 0
fi
exec "{}" "$@"
"#,
        direct_plugin.display()
    ));
    let _env = EnvVarGuard::set("IOS_CONTROL_DIRECT_RECEIVER_HELPER", &helper);

    let mut orchestrator = SessionOrchestrator::default();
    let state = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "direct-wait".into(),
            device_name: "Alice iPhone".into(),
            selected_source_id: Some("direct-1".into()),
            capture_backend: CaptureBackend::Direct,
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                capture_direct: helper.clone(),
                control_ble: plugin_path(&root, "plugin-control-ble"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    assert_eq!(state.summary.phase, SessionPhase::Connecting);
    assert_eq!(state.summary.capture_plugin.as_deref(), Some("capture.direct"));
    assert!(state.capture_stream.is_some());
    assert!(state.latest_frame.is_none());

    state.shutdown().await.unwrap();
}
