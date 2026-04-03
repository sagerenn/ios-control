use host_desktop::app::HostDesktopApp;
use host_desktop::panels::device_detail::{CaptureSourceOption, ControlSetupChecklist};
use host_desktop::view_models::session::SessionUiState;

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
        vec![CaptureSourceOption::new("window:mock", "Mock iPhone Mirror")]
    );
    assert_eq!(app.session.ui_state, SessionUiState::Idle);
    assert!(app.session.selected_source.is_none());
    assert!(app.session.latest_frame.is_none());
    assert_eq!(app.diagnostics.host_error, None);
    assert_eq!(app.diagnostics.control_summary, "control not started");
    assert_eq!(app.diagnostics.grounding_summary, "grounding idle");
    assert!(app.settings.plugin_rows.iter().any(|row| row.contains("control.ble")));
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
