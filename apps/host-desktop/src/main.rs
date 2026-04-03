fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "iOS Control Host",
        options,
        Box::new(|_cc| {
            let mut app = host_desktop::app::HostDesktopApp::new();
            if let Ok(device_id) = std::env::var("IOS_CONTROL_PENDING_START_DEVICE") {
                app.enable_runtime_start(&device_id);
            }
            Ok(Box::new(app))
        }),
    )
}
