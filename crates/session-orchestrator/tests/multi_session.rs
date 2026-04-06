use ios_control_session_orchestrator::{
    CaptureBackend, PluginPaths, SessionSupervisor, StartSessionRequest,
};

mod support;
use support::{
    build_plugins, plugin_path, prepare_window_runtime_env, runtime_env_lock, workspace_root,
};

#[tokio::test]
async fn supervisor_keeps_sessions_isolated_across_multiple_devices() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _display_guard = prepare_window_runtime_env(&root);

    let mut supervisor = SessionSupervisor::default();
    supervisor
        .start_or_replace_session(StartSessionRequest {
            device_id: "device-1".into(),
            device_name: "Device 1".into(),
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
    supervisor
        .start_or_replace_session(StartSessionRequest {
            device_id: "device-2".into(),
            device_name: "Device 2".into(),
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

    let snapshot = supervisor.session_statuses();
    assert_eq!(snapshot.len(), 2);
    assert!(snapshot.contains_key("device-1"));
    assert!(snapshot.contains_key("device-2"));
}

#[tokio::test]
async fn supervisor_retains_active_sessions_after_status_reads() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _helper_guard = prepare_window_runtime_env(&root);

    let mut supervisor = SessionSupervisor::default();
    supervisor
        .start_or_replace_session(StartSessionRequest {
            device_id: "device-1".into(),
            device_name: "Device 1".into(),
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

    let first = supervisor.session_statuses().len();
    let second = supervisor.session_statuses().len();
    assert_eq!(first, second);
    assert_eq!(second, 1);
    assert_eq!(supervisor.active_sessions().len(), 1);
}

#[tokio::test]
async fn supervisor_stop_session_clears_active_and_status_entries() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _helper_guard = prepare_window_runtime_env(&root);

    let mut supervisor = SessionSupervisor::default();
    supervisor
        .start_or_replace_session(StartSessionRequest {
            device_id: "device-stop".into(),
            device_name: "Device Stop".into(),
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

    assert!(supervisor.session_statuses().contains_key("device-stop"));
    assert!(supervisor.active_sessions().contains_key("device-stop"));

    supervisor.stop_session("device-stop").await.unwrap();

    assert!(!supervisor.session_statuses().contains_key("device-stop"));
    assert!(!supervisor.active_sessions().contains_key("device-stop"));
}
