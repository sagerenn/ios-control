fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "iOS Control Host",
        options,
        Box::new(|_cc| {
            Ok(Box::new(host_desktop::app::HostDesktopApp {
                dashboard: host_desktop::view_models::dashboard::DashboardViewModel {
                    total_devices: 0,
                    degraded_devices: 0,
                },
            }))
        }),
    )
}
