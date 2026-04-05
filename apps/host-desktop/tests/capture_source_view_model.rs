use host_desktop::panels::device_detail::CaptureSourceOption;
use host_desktop::view_models::session::{SessionUiState, SessionViewModel};

#[test]
fn capture_source_option_labels_window_and_direct_sources() {
    let window = CaptureSourceOption::new("window:airdroid", "AirDroid Window");
    let direct = CaptureSourceOption::new("direct:receiver", "Direct Receiver");
    let runtime_window = CaptureSourceOption::new("window-1", "Live Window");
    let runtime_direct = CaptureSourceOption::new("direct-1", "Live Direct");

    assert!(window.label().contains("Window"));
    assert!(direct.label().contains("Direct"));
    assert!(runtime_window.label().contains("Window"));
    assert!(runtime_direct.label().contains("Direct"));
}

#[test]
fn session_view_model_actions_follow_ui_state() {
    let idle = SessionViewModel::idle();
    assert_eq!(idle.ui_state, SessionUiState::Idle);
    assert!(!idle.can_start());
    assert!(!idle.can_stop());
    assert_eq!(idle.status_line(), "No active session");

    let idle_startable = SessionViewModel::idle_startable(None);
    assert_eq!(idle_startable.ui_state, SessionUiState::Idle);
    assert!(idle_startable.can_start());
    assert!(!idle_startable.can_stop());

    let starting = SessionViewModel::starting();
    assert_eq!(starting.ui_state, SessionUiState::Starting);
    assert!(!starting.can_start());
    assert!(!starting.can_stop());
    assert_eq!(starting.status_line(), "Starting session");

    let error = SessionViewModel::error("Missing backend");
    assert_eq!(error.ui_state, SessionUiState::Error("Missing backend".into()));
    assert!(error.can_start());
    assert!(!error.can_stop());
    assert_eq!(error.status_line(), "Missing backend");

    let blocked = SessionViewModel::blocked("No capture path observed", None);
    assert_eq!(
        blocked.ui_state,
        SessionUiState::Error("No capture path observed".into())
    );
    assert!(!blocked.can_start());
    assert!(!blocked.can_stop());
}
