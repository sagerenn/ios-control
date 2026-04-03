fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "iOS Control Host",
        options,
        Box::new(|_cc| Ok(Box::new(host_desktop::app::HostDesktopApp::new()))),
    )
}
