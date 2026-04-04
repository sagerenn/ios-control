use host_desktop::runtime::{HostRuntime, HostRuntimeConfig};
use ios_control_contracts::session::SessionPhase;

mod support;
use support::{
    build_plugins, host_plugin_paths, prepare_window_runtime_env, runtime_env_lock, workspace_root,
};

#[test]
fn runtime_start_session_returns_workspace_snapshot() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _guards = prepare_window_runtime_env(&root);

    let mut runtime = HostRuntime::new(HostRuntimeConfig {
        plugin_paths: host_plugin_paths(&root),
    })
    .unwrap();

    let snapshot = runtime
        .start_session("device-1", "Mock iPhone", Some("window-helper-1".into()))
        .unwrap();

    assert_eq!(snapshot.statuses.len(), 1);
    assert_eq!(snapshot.workspace.device_id, "device-1");
    assert!(matches!(
        snapshot.workspace.summary.phase,
        SessionPhase::Streaming | SessionPhase::Degraded
    ));
    assert_eq!(snapshot.workspace.capture_sources.len(), 1);
}
