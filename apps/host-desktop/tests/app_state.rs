use host_desktop::app::HostDesktopApp;
use host_desktop::view_models::dashboard::DashboardViewModel;
use host_desktop::view_models::device_detail::DeviceDetailViewModel;
use host_desktop::view_models::diagnostics::DiagnosticsViewModel;
use host_desktop::view_models::session::SessionViewModel;
use host_desktop::view_models::settings::SettingsViewModel;

#[test]
fn host_app_exposes_end_to_end_demo_state() {
    let _ = (
        std::mem::size_of::<DashboardViewModel>(),
        std::mem::size_of::<DeviceDetailViewModel>(),
        std::mem::size_of::<DiagnosticsViewModel>(),
        std::mem::size_of::<SessionViewModel>(),
        std::mem::size_of::<SettingsViewModel>(),
    );

    let app = HostDesktopApp::demo();

    assert_eq!(app.dashboard.total_devices, 1);
    assert_eq!(app.dashboard.degraded_devices, 0);
    assert_eq!(app.device_detail.device_name, "Mock iPhone");
    assert_eq!(app.session.selected_source_label, "Window: Mock iPhone Mirror");
    assert!(app.diagnostics.grounding_summary.contains("selected"));
    assert!(app.settings.plugin_rows.iter().any(|row| row.contains("control.ble")));
}
