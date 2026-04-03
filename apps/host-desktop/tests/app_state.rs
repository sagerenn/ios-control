use host_desktop::app::HostDesktopApp;
use host_desktop::panels::device_detail::{CaptureSourceOption, ControlSetupChecklist};

#[test]
fn host_app_exposes_end_to_end_demo_state() {
    let app = HostDesktopApp::demo();

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
    assert!(app.session.selected_source.is_none());
    assert!(app.session.latest_frame.is_none());
    assert_eq!(app.diagnostics.control_summary, "control not started");
    assert_eq!(app.diagnostics.grounding_summary, "grounding idle");
    assert!(app.settings.plugin_rows.iter().any(|row| row.contains("control.ble")));
}
