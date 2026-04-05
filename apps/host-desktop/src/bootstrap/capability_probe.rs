use ios_control_session_orchestrator::PluginPaths;

use crate::inventory::providers::{
    list_capture_sources, probe_capture_capability, probe_control_capability,
};
use crate::view_models::startup::{StartupItem, StartupReadiness, StartupViewModel};

pub fn startup_from_plugin_paths(plugin_paths: &PluginPaths) -> StartupViewModel {
    let mut items = vec![
        probe_capture_item("Window Capture", &plugin_paths.capture),
        probe_control_item("BLE Control", &plugin_paths.control_ble),
        probe_control_item("Window Input Bridge", &plugin_paths.control_fallback),
    ];
    if let Some(path) = plugin_paths.grounding.as_ref() {
        items.push(item_for_path("Grounding", path));
    }

    let capture_ready = items
        .iter()
        .any(|item| item.label == "Window Capture" && item.status == "Ready");
    let control_ready = items.iter().any(|item| {
        matches!(item.label.as_str(), "BLE Control" | "Window Input Bridge") && item.status == "Ready"
    });
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

fn probe_capture_item(label: &str, path: &std::path::Path) -> StartupItem {
    if !path.is_file() {
        return item_for_path(label, path);
    }

    match probe_capture_capability(path) {
        Ok(capability) if !capability.available => StartupItem {
            label: label.into(),
            status: "Blocked".into(),
            detail: capability
                .reason
                .unwrap_or_else(|| "capture probe reported unavailable".into()),
        },
        Ok(capability) => match list_capture_sources(path) {
            Ok(sources) if !sources.is_empty() => StartupItem {
                label: label.into(),
                status: "Ready".into(),
                detail: format!(
                    "{} | {} source{} discovered",
                    capability.backend_id,
                    sources.len(),
                    if sources.len() == 1 { "" } else { "s" }
                ),
            },
            Ok(_) => StartupItem {
                label: label.into(),
                status: "Blocked".into(),
                detail: format!("{} | no capture sources discovered", capability.backend_id),
            },
            Err(error) => StartupItem {
                label: label.into(),
                status: "Blocked".into(),
                detail: format!("{} | source listing failed: {error}", capability.backend_id),
            },
        },
        Err(error) => StartupItem {
            label: label.into(),
            status: "Error".into(),
            detail: format!("capture probe failed: {error}"),
        },
    }
}

fn probe_control_item(label: &str, path: &std::path::Path) -> StartupItem {
    if !path.is_file() {
        return item_for_path(label, path);
    }

    match probe_control_capability(path) {
        Ok(capability) if capability.supported => StartupItem {
            label: label.into(),
            status: "Ready".into(),
            detail: format!("{:?} | helper ready", capability.transport),
        },
        Ok(capability) => StartupItem {
            label: label.into(),
            status: "Blocked".into(),
            detail: capability
                .reason
                .unwrap_or_else(|| "control probe reported unavailable".into()),
        },
        Err(error) => StartupItem {
            label: label.into(),
            status: "Error".into(),
            detail: format!("control probe failed: {error}"),
        },
    }
}

fn item_for_path(label: &str, path: &std::path::Path) -> StartupItem {
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
