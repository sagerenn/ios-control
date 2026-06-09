use std::path::PathBuf;

use host_desktop::bootstrap::bootstrap_startup;
use host_desktop::preferences::HostPreferencesStore;
use host_desktop::runtime::HostRuntimeConfig;

fn main() -> eframe::Result<()> {
    let bootstrap = bootstrap_startup(
        std::env::current_exe().expect("current exe path should resolve"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    )
    .expect("startup bootstrap should resolve");
    let runtime_config = HostRuntimeConfig {
        plugin_paths: bootstrap.layout.plugin_paths.clone(),
    };
    let startup = bootstrap.startup.clone();
    let preferences_path = HostPreferencesStore::default_path();

    eframe::run_native(
        "iOS Control Host",
        eframe::NativeOptions::default(),
        Box::new(move |_cc| {
            let mut app = if let Some(path) = preferences_path.clone() {
                host_desktop::app::HostDesktopApp::with_runtime_and_preferences(
                    runtime_config.clone(),
                    HostPreferencesStore::new(path),
                )
            } else {
                host_desktop::app::HostDesktopApp::with_runtime(runtime_config.clone())
            };
            app.apply_startup_view(startup.clone());

            if let Ok(device_id) = std::env::var("IOS_CONTROL_PENDING_START_DEVICE") {
                app.set_pending_start_device(device_id);
            }
            Ok(Box::new(app))
        }),
    )
}
