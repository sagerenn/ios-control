use std::path::Path;

use ios_control_session_orchestrator::PluginPaths;

use crate::view_models::startup::{StartupItem, StartupReadiness, StartupViewModel};

pub fn startup_from_plugin_paths(plugin_paths: &PluginPaths) -> StartupViewModel {
    let mut items = vec![
        item_for_path("Window Capture", &plugin_paths.capture),
        item_for_path("BLE Control", &plugin_paths.control_ble),
        item_for_path("Window Input Bridge", &plugin_paths.control_fallback),
    ];
    if let Some(path) = plugin_paths.grounding.as_ref() {
        items.push(item_for_path("Grounding", path));
    }

    let capture_ready = plugin_paths.capture.is_file();
    let control_ready = plugin_paths.control_ble.is_file() || plugin_paths.control_fallback.is_file();
    let readiness = if capture_ready && control_ready {
        StartupReadiness::Ready
    } else if capture_ready || control_ready {
        StartupReadiness::Partial
    } else {
        StartupReadiness::Blocked
    };

    let summary = match readiness {
        StartupReadiness::Ready => "Ready: runtime components resolved".into(),
        StartupReadiness::Partial => "Partial: some runtime components are unavailable".into(),
        StartupReadiness::Blocked => "Blocked: no usable device path yet".into(),
    };

    StartupViewModel {
        readiness,
        summary,
        items,
    }
}

fn item_for_path(label: &str, path: &Path) -> StartupItem {
    StartupItem {
        label: label.into(),
        status: if path.is_file() {
            "Ready".into()
        } else {
            "Missing".into()
        },
        detail: path.display().to_string(),
    }
}
