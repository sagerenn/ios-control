use std::path::PathBuf;

use host_desktop::runtime::{HostRuntime, HostRuntimeConfig};
use ios_control_contracts::session::SessionPhase;

mod support;
use support::{
    build_plugins, host_plugin_paths, prepare_window_runtime_env, runtime_env_lock, target_dir,
    workspace_root,
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
    assert_eq!(
        snapshot.statuses[0].summary().device_id,
        snapshot.workspace.device_id
    );
    assert_eq!(snapshot.workspace.device_id, "device-1");
    assert!(matches!(
        snapshot.workspace.summary.phase,
        SessionPhase::Streaming | SessionPhase::Degraded
    ));
    assert_eq!(snapshot.workspace.capture_sources.len(), 1);
}

#[test]
fn runtime_refresh_session_updates_workspace_latest_frame() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _guards = prepare_window_runtime_env(&root);

    let mut runtime = HostRuntime::new(HostRuntimeConfig {
        plugin_paths: host_plugin_paths(&root),
    })
    .unwrap();

    let first = runtime
        .start_session("device-1", "Mock iPhone", Some("window-helper-1".into()))
        .unwrap()
        .workspace
        .latest_frame
        .unwrap()
        .frame_index;

    let refreshed = runtime.refresh_session("device-1").unwrap();
    assert!(refreshed.workspace.latest_frame.unwrap().frame_index > first);
}

#[test]
fn runtime_bootstrap_uses_repo_layout_without_env_vars() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);

    let bootstrap = host_desktop::bootstrap::bootstrap_startup(
        target_dir(&root).join(format!("debug/host-desktop{}", std::env::consts::EXE_SUFFIX)),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();

    assert!(bootstrap
        .layout
        .plugin_paths
        .capture
        .ends_with(format!("plugin-capture-window{}", std::env::consts::EXE_SUFFIX)));
    assert!(!bootstrap.startup.summary.is_empty());
}
