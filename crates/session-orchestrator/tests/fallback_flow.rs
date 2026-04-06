use ios_control_session_orchestrator::{
    CaptureBackend, PluginPaths, SessionSupervisor, StartSessionRequest,
};

mod support;
use support::{
    build_plugins, plugin_path, prepare_window_runtime_env, runtime_env_lock, workspace_root,
};

#[tokio::test]
async fn supervisor_uses_window_fallback_when_ble_backend_is_unavailable() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _display_guard = prepare_window_runtime_env(&root);
    std::env::remove_var("IOS_CONTROL_BLE_HELPER");
    std::env::set_var(
        "IOS_CONTROL_WINDOW_INPUT_HELPER",
        plugin_path(&root, "plugin-control-window-bridge"),
    );

    let mut supervisor = SessionSupervisor::default();
    let status = supervisor
        .start_or_replace_session(StartSessionRequest {
            device_id: "device-ble-fallback".into(),
            device_name: "Fallback iPhone".into(),
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

    assert_eq!(status.backends().control_backend, "control.window-bridge");
}

#[tokio::test]
async fn supervisor_uses_window_fallback_when_ble_backend_startup_fails() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _display_guard = prepare_window_runtime_env(&root);
    std::env::set_var(
        "IOS_CONTROL_WINDOW_INPUT_HELPER",
        plugin_path(&root, "plugin-control-window-bridge"),
    );

    let mut supervisor = SessionSupervisor::default();
    let status = supervisor
        .start_or_replace_session(StartSessionRequest {
            device_id: "device-ble-spawn-fallback".into(),
            device_name: "Fallback iPhone".into(),
            selected_source_id: Some("window-helper-1".into()),
            capture_backend: CaptureBackend::Window,
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                capture_direct: plugin_path(&root, "plugin-capture-direct"),
                control_ble: plugin_path(&root, "plugin-control-ble-missing"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    assert_eq!(status.backends().control_backend, "control.window-bridge");
}
