use host_desktop::app::HostDesktopApp;
use host_desktop::panels::device_detail::{CaptureSourceOption, ControlSetupChecklist};
use host_desktop::runtime::{HostRuntimeConfig, HostRuntimeSnapshot, RuntimeWorkspaceState};
use host_desktop::view_models::session::SessionUiState;
use ios_control_contracts::control::ControlSessionPhase;
use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{
    BackendSelection, DeviceSessionStatus, DeviceSessionSummary, SessionPhase, SessionSubstate,
};
use ios_control_session_orchestrator::SessionDiagnostics;

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

fn runtime_snapshot_with_control(
    control_phase: ControlSessionPhase,
    control_summary: &str,
    execution_observed_change: Option<bool>,
) -> HostRuntimeSnapshot {
    let status = status(
        "device-1",
        "Alpha",
        SessionPhase::Degraded,
        SessionSubstate::OperatorActionRequired,
        "capture.window.helper",
        "control.ble",
        Some("Reconnect BLE helper"),
    );

    HostRuntimeSnapshot {
        statuses: vec![status.clone()],
        workspace: RuntimeWorkspaceState {
            device_id: "device-1".into(),
            summary: status.summary().clone(),
            capture_sources: vec![ios_control_contracts::capture::VideoSource {
                source_id: "window-helper-1".into(),
                display_name: "Operator Mirror".into(),
                kind: ios_control_contracts::capture::SourceKind::Window,
            }],
            capture_stream: None,
            selected_source_id: Some("window-helper-1".into()),
            control_checklist: ios_control_contracts::control::ControlSetupChecklist {
                items: vec!["Pair the device".into()],
            },
            control_phase,
            execution_observed_change,
            diagnostics: SessionDiagnostics {
                control_phase,
                control_summary: control_summary.into(),
                grounding_summary: Some("selected pointer plan".into()),
            },
        },
    }
}

fn host_app_from_runtime_snapshot(snapshot: HostRuntimeSnapshot) -> HostDesktopApp {
    let mut app = HostDesktopApp::new();
    app.apply_runtime_snapshot(snapshot);
    app
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
fn host_app_without_runtime_reports_runtime_unavailable() {
    let mut app = HostDesktopApp::new();

    app.request_start_session();
    assert_eq!(
        app.session.ui_state,
        SessionUiState::Error("Host runtime unavailable".into())
    );
    assert!(app.session.selected_source.is_none());
    assert!(app.session.latest_frame.is_none());
    assert_eq!(app.device_detail.active_source_id, None);
    assert_eq!(
        app.diagnostics.host_error.as_deref(),
        Some("Host runtime unavailable")
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
fn host_app_without_runtime_stays_unavailable_even_without_capture_sources() {
    let mut app = HostDesktopApp::new();
    app.device_detail.capture_sources.clear();

    app.request_start_session();

    assert_eq!(
        app.session.ui_state,
        SessionUiState::Error("Host runtime unavailable".into())
    );
    assert!(app.session.selected_source.is_none());
    assert!(app.session.latest_frame.is_none());
    assert_eq!(
        app.diagnostics.host_error.as_deref(),
        Some("Host runtime unavailable")
    );
    assert!(app.diagnostics.control_summary.contains("blocked"));
    assert!(app.diagnostics.grounding_summary.contains("blocked"));
}

#[test]
fn start_session_without_runtime_with_selected_device_reports_runtime_unavailable() {
    let mut app = HostDesktopApp::new();
    app.select_device("device-1");

    app.request_start_session();

    assert_eq!(
        app.session.ui_state,
        SessionUiState::Error("Host runtime unavailable".into())
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
        Some("Host runtime unavailable")
    );
    assert!(!fixture.app.device_detail.capture_sources.is_empty());
}

#[test]
fn host_app_start_session_forwards_selected_source_to_runtime() {
    let mut fixture = host_app_with_runtime();
    fixture.app.select_device("device-1");
    fixture.app.device_detail.capture_sources = vec![CaptureSourceOption::new(
        "missing-source",
        "Broken Source",
    )];
    fixture.app.device_detail.active_source_id = Some("missing-source".into());

    fixture.app.request_start_session();

    match &fixture.app.session.ui_state {
        SessionUiState::Error(message) => {
            assert!(message.contains("missing-source"));
            assert!(message.contains("unavailable"));
        }
        other => panic!("expected runtime start failure, got {other:?}"),
    }
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
fn selecting_a_capture_source_updates_device_detail_selection() {
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
    assert!(fixture.app.diagnostics.control_summary.contains("control "));
}

#[test]
fn runtime_snapshot_preserves_control_phase_and_observed_change() {
    let snapshot = runtime_snapshot_with_control(
        ControlSessionPhase::Advertising,
        "Waiting for iPhone",
        Some(true),
    );

    assert_eq!(snapshot.workspace.control_phase, ControlSessionPhase::Advertising);
    assert_eq!(snapshot.workspace.execution_observed_change, Some(true));
}

#[test]
fn host_app_surfaces_reconnect_guidance_for_degraded_control() {
    let app = host_app_from_runtime_snapshot(runtime_snapshot_with_control(
        ControlSessionPhase::Error,
        "Reconnect BLE helper",
        Some(false),
    ));

    assert!(app.diagnostics.control_summary.contains("Reconnect BLE helper"));
}

#[test]
fn startup_with_selected_device_attempts_start_and_reports_runtime_unavailable() {
    let mut app = HostDesktopApp::new();
    app.select_device("device-1");

    app.start_runtime_session_on_launch();

    assert_eq!(app.selected_device_id.as_deref(), Some("device-1"));
    assert_eq!(
        app.session.ui_state,
        SessionUiState::Error("Host runtime unavailable".into())
    );
}

#[test]
fn stop_session_clears_runtime_state_even_without_runtime_instance() {
    let mut app = HostDesktopApp::new();
    app.replace_runtime_statuses(vec![status(
        "device-1",
        "Alpha",
        SessionPhase::Streaming,
        SessionSubstate::Streaming,
        "capture.window.helper",
        "control.ble",
        None,
    )]);

    app.stop_session();

    assert!(app.available_device_ids.is_empty());
    assert!(app.fleet.rows.is_empty());
    assert_eq!(app.dashboard.total_devices, 0);
    assert_eq!(app.dashboard.degraded_devices, 0);
    assert!(app.settings.plugin_rows.is_empty());
    assert!(app.selected_device_id.is_none());
    assert_eq!(app.session.ui_state, SessionUiState::Idle);
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
