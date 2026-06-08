use std::path::Path;

use anyhow::anyhow;
use ios_control_contracts::capture::CaptureCapability;
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use ios_control_plugin_runtime::RunningPlugin;
use ios_control_session_orchestrator::PluginPaths;

use crate::inventory::providers::{
    list_capture_sources, probe_capture_capability, probe_control_capability,
};
use crate::view_models::startup::{
    DirectReceiverViewModel, StartupItem, StartupReadiness, StartupViewModel,
};

pub fn startup_from_plugin_paths(plugin_paths: &PluginPaths) -> StartupViewModel {
    let window_capture = probe_capture_item("Window Capture", &plugin_paths.capture);
    let direct_receiver = probe_direct_receiver(
        &plugin_paths.capture_direct,
        plugin_paths.capture_direct_runtime_root.as_deref(),
    );
    let mut items = vec![
        window_capture,
        probe_control_item("BLE Control", &plugin_paths.control_ble),
        probe_control_item("Window Input Bridge", &plugin_paths.control_fallback),
    ];
    if let Some(path) = plugin_paths.grounding.as_ref() {
        items.push(item_for_path("Grounding", path));
    }

    let capture_ready = direct_receiver.available
        || items
            .iter()
            .any(|item| item.label == "Window Capture" && item.status == "Ready");
    let control_ready = items.iter().any(|item| {
        matches!(item.label.as_str(), "BLE Control" | "Window Input Bridge")
            && item.status == "Ready"
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
        direct_receiver,
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

fn probe_direct_receiver(path: &Path, runtime_root: Option<&Path>) -> DirectReceiverViewModel {
    if !path.is_file() {
        return DirectReceiverViewModel {
            available: false,
            status: "Missing".into(),
            detail: path.display().to_string(),
        };
    }

    match probe_direct_capture_capability(path, runtime_root) {
        Ok(capability) if capability.available => DirectReceiverViewModel {
            available: true,
            status: "Ready".into(),
            detail: format!("{} | receiver ready", capability.backend_id),
        },
        Ok(capability) => DirectReceiverViewModel {
            available: false,
            status: "Blocked".into(),
            detail: capability
                .reason
                .unwrap_or_else(|| "capture probe reported unavailable".into()),
        },
        Err(error) => DirectReceiverViewModel {
            available: false,
            status: "Error".into(),
            detail: format!("capture probe failed: {error}"),
        },
    }
}

fn probe_direct_capture_capability(
    path: &Path,
    runtime_root: Option<&Path>,
) -> anyhow::Result<CaptureCapability> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let envs = runtime_root
            .map(|root| {
                vec![(
                    "IOS_CONTROL_DIRECT_RUNTIME_ROOT".to_string(),
                    root.as_os_str().to_owned(),
                )]
            })
            .unwrap_or_default();
        let mut plugin = RunningPlugin::spawn_with_env(path, envs).await?;
        plugin.handshake().await?;
        plugin.send(&HostToPlugin::ProbeCapture).await?;
        let reply = plugin.read().await?;
        let _ = plugin.stop().await;
        match reply {
            PluginToHost::CaptureCapability { capability } => Ok(capability),
            other => Err(anyhow!("unexpected capture capability response: {other:?}")),
        }
    })
}
