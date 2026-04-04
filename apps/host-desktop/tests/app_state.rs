use host_desktop::app::HostDesktopApp;
use host_desktop::panels::device_detail::{CaptureSourceOption, ControlSetupChecklist};
use host_desktop::runtime::HostRuntimeConfig;
use host_desktop::view_models::session::SessionUiState;
use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{
    BackendSelection, DeviceSessionStatus, DeviceSessionSummary, SessionPhase, SessionSubstate,
};

mod support;
use support::{
    build_plugins, host_plugin_paths, prepare_window_runtime_env, runtime_env_lock, workspace_root,
    EnvVarGuards,
};

struct RuntimeAppFixture {
    _lock: std::sync::MutexGuard<'static, ()>,
    _guards: EnvVarGuards,
    app: HostDesktopApp,
}

fn host_app_with_runtime() -> RuntimeAppFixture {
    let lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let guards = prepare_window_runtime_env(&root);
    let app = HostDesktopApp::with_runtime(HostRuntimeConfig {
        plugin_paths: host_plugin_paths(&root),
    });

    RuntimeAppFixture {
        _lock: lock,
        _guards: guards,
        app,
    }
}

#[test]
fn host_app_boots_into_an_honest_idle_session_shell() {
    let app = HostDesktopApp::new();

    assert_eq!(app.dashboard.total_devices, 1);
    assert_eq!(app.dashboard.degraded_devices, 0);
    assert_eq!(app.device_detail.device_name, "Mock iPhone");
    assert_eq!(
        app.device_detail.control_checklist,
        ControlSetupChecklist::for_pointer_mode()
    );
    assert_eq!(
        app.device_detail.capture_sources,
        vec![CaptureSourceOption::new(
            "window:mock",
            "Mock iPhone Mirror"
        )]
    );
    assert_eq!(app.session.ui_state, SessionUiState::Idle);
    assert!(app.session.selected_source.is_none());
    assert!(app.session.latest_frame.is_none());
    assert_eq!(app.diagnostics.host_error, None);
    assert_eq!(app.diagnostics.control_summary, "control not started");
    assert_eq!(app.diagnostics.grounding_summary, "grounding idle");
    assert!(app
        .settings
        .plugin_rows
        .iter()
        .any(|row| row.contains("control.ble")));
}

#[test]
fn host_app_transitions_from_starting_to_honest_bootstrap_error() {
    let mut app = HostDesktopApp::new();

    app.request_start_session();
    assert_eq!(app.session.ui_state, SessionUiState::Starting);
    assert!(app.session.selected_source.is_none());
    assert!(app.session.latest_frame.is_none());

    app.finish_pending_session_start();
    assert_eq!(
        app.session.ui_state,
        SessionUiState::Error("Session bootstrap is not wired to the runtime yet".into())
    );
    assert!(app.session.selected_source.is_none());
    assert!(app.session.latest_frame.is_none());
    assert_eq!(app.device_detail.active_source_id, None);
    assert_eq!(
        app.diagnostics.host_error.as_deref(),
        Some("Session bootstrap is not wired to the runtime yet")
    );
    assert_eq!(app.diagnostics.control_summary, "control blocked");
    assert_eq!(app.diagnostics.grounding_summary, "grounding blocked");

    app.stop_session();
    assert_eq!(app.session.ui_state, SessionUiState::Idle);
    assert!(app.session.selected_source.is_none());
    assert!(app.session.latest_frame.is_none());
    assert_eq!(app.device_detail.active_source_id, None);
    assert_eq!(app.diagnostics.control_summary, "control not started");
    assert_eq!(app.diagnostics.grounding_summary, "grounding idle");
}

#[test]
fn host_app_surfaces_bootstrap_errors_when_no_capture_source_exists() {
    let mut app = HostDesktopApp::new();
    app.device_detail.capture_sources.clear();

    app.request_start_session();
    app.finish_pending_session_start();

    assert_eq!(
        app.session.ui_state,
        SessionUiState::Error("No capture sources available".into())
    );
    assert!(app.session.selected_source.is_none());
    assert!(app.session.latest_frame.is_none());
    assert_eq!(
        app.diagnostics.host_error.as_deref(),
        Some("No capture sources available")
    );
    assert!(app.diagnostics.control_summary.contains("blocked"));
    assert!(app.diagnostics.grounding_summary.contains("blocked"));
}

#[test]
fn start_session_uses_runtime_bridge_instead_of_bootstrap_error() {
    let mut app = HostDesktopApp::new();
    app.enable_runtime_start("device-1");

    app.request_start_session();
    app.finish_pending_session_start();

    assert_ne!(
        app.session.ui_state,
        SessionUiState::Error("Session bootstrap is not wired to the runtime yet".into())
    );
}

#[test]
fn host_app_start_session_uses_real_runtime_snapshot() {
    let mut fixture = host_app_with_runtime();
    fixture.app.select_device("device-1");

    fixture.app.request_start_session();

    assert!(matches!(
        fixture.app.session.ui_state,
        SessionUiState::Streaming | SessionUiState::Error(_)
    ));
    assert_ne!(
        fixture.app.diagnostics.host_error.as_deref(),
        Some("Session bootstrap is not wired to the runtime yet")
    );
}

#[test]
fn host_app_stop_session_removes_runtime_status() {
    let mut fixture = host_app_with_runtime();
    fixture.app.select_device("device-1");
    fixture.app.request_start_session();

    fixture.app.stop_session();

    assert!(fixture.app.available_device_ids.is_empty());
    assert_eq!(fixture.app.session.ui_state, SessionUiState::Idle);
}

#[test]
fn selecting_a_capture_source_updates_runtime_selection() {
    let mut fixture = host_app_with_runtime();
    fixture.app.select_device("device-1");
    fixture.app.request_start_session();

    fixture.app.select_capture_source("window-helper-1");

    assert_eq!(
        fixture.app.device_detail.active_source_id.as_deref(),
        Some("window-helper-1")
    );
}

#[test]
fn runtime_snapshot_populates_control_checklist_and_operator_message() {
    let mut fixture = host_app_with_runtime();
    fixture.app.select_device("device-1");
    fixture.app.request_start_session();

    assert_ne!(
        fixture.app.device_detail.control_checklist,
        ControlSetupChecklist::for_pointer_mode()
    );
    assert_ne!(
        fixture.app.diagnostics.control_summary,
        "control backend control.ble"
    );
    assert!(fixture.app.diagnostics.control_summary.starts_with("control "));
}

#[test]
fn startup_runtime_queue_advances_start_path_on_launch() {
    let mut app = HostDesktopApp::new();
    app.enable_runtime_start("device-1");

    app.start_runtime_session_on_launch();

    assert_eq!(app.selected_device_id.as_deref(), Some("device-1"));
    assert_eq!(app.session.ui_state, SessionUiState::Starting);
    assert_ne!(
        app.session.ui_state,
        SessionUiState::Error("Session bootstrap is not wired to the runtime yet".into())
    );
}

#[test]
fn app_tracks_selected_workspace_separately_from_fleet_rows() {
    let mut app = HostDesktopApp::new();
    app.selected_device_id = Some("device-2".into());
    app.available_device_ids = vec!["device-1".into(), "device-2".into()];

    assert_eq!(app.selected_device_id.as_deref(), Some("device-2"));
    assert_eq!(app.available_device_ids.len(), 2);
}

fn status(
    device_id: &str,
    device_name: &str,
    phase: SessionPhase,
    substate: SessionSubstate,
    capture_backend: &str,
    control_backend: &str,
    operator_action: Option<&str>,
) -> DeviceSessionStatus {
    DeviceSessionStatus::new(
        DeviceSessionSummary {
            device_id: device_id.into(),
            device_name: device_name.into(),
            phase,
            plugin_health: if phase == SessionPhase::Degraded {
                PluginHealth::Degraded
            } else {
                PluginHealth::Healthy
            },
            capture_plugin: Some(capture_backend.into()),
            control_plugin: Some(control_backend.into()),
            grounding_plugin: Some("grounding.core".into()),
        },
        substate,
        BackendSelection {
            capture_backend: capture_backend.into(),
            control_backend: control_backend.into(),
        },
        operator_action.map(str::to_string),
    )
    .expect("valid session status")
}

#[test]
fn app_syncs_runtime_statuses_into_fleet_and_workspace() {
    let mut app = HostDesktopApp::new();
    app.replace_runtime_statuses(vec![
        status(
            "device-1",
            "Alpha",
            SessionPhase::Streaming,
            SessionSubstate::ControlReady,
            "capture.window.helper",
            "control.ble",
            None,
        ),
        status(
            "device-2",
            "Beta",
            SessionPhase::Degraded,
            SessionSubstate::OperatorActionRequired,
            "capture.window.helper",
            "control.window-bridge",
            Some("reconnect mirror helper"),
        ),
    ]);

    assert_eq!(app.dashboard.total_devices, 2);
    assert_eq!(app.dashboard.degraded_devices, 1);
    assert_eq!(app.available_device_ids, vec!["device-1", "device-2"]);
    assert_eq!(app.selected_device_id.as_deref(), Some("device-1"));
    assert_eq!(app.fleet.rows.len(), 2);
    assert_eq!(app.device_detail.device_name, "Alpha");
    assert_eq!(app.session.ui_state, SessionUiState::Streaming);
    assert!(app.diagnostics.host_error.is_none());
    assert!(app
        .settings
        .plugin_rows
        .iter()
        .any(|row| row.contains("control.window-bridge")));
}

#[test]
fn selecting_a_device_updates_workspace_and_operator_error() {
    let mut app = HostDesktopApp::new();
    app.replace_runtime_statuses(vec![
        status(
            "device-1",
            "Alpha",
            SessionPhase::Streaming,
            SessionSubstate::ControlReady,
            "capture.window.helper",
            "control.ble",
            None,
        ),
        status(
            "device-2",
            "Beta",
            SessionPhase::Degraded,
            SessionSubstate::OperatorActionRequired,
            "capture.window.helper",
            "control.window-bridge",
            Some("reconnect mirror helper"),
        ),
    ]);

    app.select_device("device-2");

    assert_eq!(app.selected_device_id.as_deref(), Some("device-2"));
    assert_eq!(app.device_detail.device_name, "Beta");
    assert_eq!(
        app.session.ui_state,
        SessionUiState::Error("reconnect mirror helper".into())
    );
    assert_eq!(
        app.diagnostics.host_error.as_deref(),
        Some("reconnect mirror helper")
    );
    assert!(app
        .diagnostics
        .control_summary
        .contains("control.window-bridge"));
}
