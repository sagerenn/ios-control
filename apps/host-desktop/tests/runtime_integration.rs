use std::path::PathBuf;

use host_desktop::runtime::{HostRuntime, HostRuntimeConfig};
use ios_control_contracts::capture::CaptureStreamPhase;
use ios_control_contracts::session::SessionPhase;
use ios_control_session_orchestrator::CaptureBackend;

mod support;
use support::{
    build_plugins, host_plugin_paths, prepare_window_runtime_env, runtime_env_lock, target_dir,
    workspace_root, EnvVarGuards, EnvVarGuard,
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
        .start_session(
            "device-1",
            "Mock iPhone",
            Some("window-helper-1".into()),
            CaptureBackend::Window,
        )
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
        .start_session(
            "device-1",
            "Mock iPhone",
            Some("window-helper-1".into()),
            CaptureBackend::Window,
        )
        .unwrap()
        .workspace
        .latest_frame
        .unwrap()
        .frame_index;

    let refreshed = runtime.refresh_session("device-1").unwrap();
    assert!(refreshed.workspace.latest_frame.unwrap().frame_index > first);
}

#[test]
fn runtime_snapshot_exposes_capture_status_for_direct_sessions() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _guards = EnvVarGuards::new(vec![EnvVarGuard::set(
        "IOS_CONTROL_DIRECT_RECEIVER_HELPER",
        support::plugin_path(&root, "plugin-capture-direct"),
    )]);

    let mut runtime = HostRuntime::new(HostRuntimeConfig {
        plugin_paths: host_plugin_paths(&root),
    })
    .unwrap();

    let snapshot = runtime
        .start_session(
            "device-1",
            "Mock iPhone",
            Some("direct-1".into()),
            CaptureBackend::Direct,
        )
        .unwrap();

    let capture_status = snapshot
        .workspace
        .capture_status
        .expect("direct runtime should expose capture status");
    assert_eq!(capture_status.video_phase, CaptureStreamPhase::Streaming);
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

#[test]
fn runtime_bootstrap_probes_embedded_window_helpers_without_env_vars() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _guards = EnvVarGuards::new(vec![
        EnvVarGuard::remove("IOS_CONTROL_WINDOW_CAPTURE_HELPER"),
        EnvVarGuard::remove("IOS_CONTROL_WINDOW_INPUT_HELPER"),
    ]);

    let bootstrap = host_desktop::bootstrap::bootstrap_startup(
        target_dir(&root).join(format!("debug/host-desktop{}", std::env::consts::EXE_SUFFIX)),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();

    let capture = bootstrap
        .startup
        .items
        .iter()
        .find(|item| item.label == "Window Capture")
        .expect("window capture startup item should exist");
    assert_eq!(capture.status, "Ready");
    assert!(
        capture.detail.contains("source"),
        "expected capture probe detail to mention discovered sources, got {}",
        capture.detail
    );

    let fallback = bootstrap
        .startup
        .items
        .iter()
        .find(|item| item.label == "Window Input Bridge")
        .expect("window input startup item should exist");
    assert_eq!(fallback.status, "Ready");
    assert!(
        fallback.detail.contains("helper") || fallback.detail.contains("control.window-bridge"),
        "expected fallback probe detail to mention helper resolution, got {}",
        fallback.detail
    );
}
