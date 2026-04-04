use std::path::{Path, PathBuf};

use host_desktop::runtime::HostRuntimeConfig;
use ios_control_session_orchestrator::PluginPaths;

fn main() -> eframe::Result<()> {
    let runtime_config = runtime_config();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "iOS Control Host",
        options,
        Box::new(move |_cc| {
            let mut app = host_desktop::app::HostDesktopApp::with_runtime(runtime_config.clone());
            if let Ok(device_id) = std::env::var("IOS_CONTROL_PENDING_START_DEVICE") {
                app.enable_runtime_start(&device_id);
            }
            app.start_runtime_session_on_launch();
            Ok(Box::new(app))
        }),
    )
}

fn runtime_config() -> HostRuntimeConfig {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should exist")
        .to_path_buf();
    HostRuntimeConfig {
        plugin_paths: PluginPaths {
            capture: plugin_path(&workspace_root, "plugin-capture-window"),
            control_ble: plugin_path(&workspace_root, "plugin-control-ble"),
            control_fallback: plugin_path(&workspace_root, "plugin-control-window-bridge"),
            grounding: Some(plugin_path(&workspace_root, "plugin-grounding-core")),
        },
    }
}

fn plugin_path(workspace_root: &Path, name: &str) -> PathBuf {
    let mut target_dir = match std::env::var_os("CARGO_TARGET_DIR") {
        Some(path) => {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                workspace_root.join(path)
            }
        }
        None => workspace_root.join("target"),
    };

    if let Some(target) = std::env::var_os("CARGO_BUILD_TARGET") {
        target_dir.push(target);
    }

    target_dir.join(format!("debug/{}{}", name, std::env::consts::EXE_SUFFIX))
}
