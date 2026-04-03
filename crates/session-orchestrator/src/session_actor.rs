use anyhow::{anyhow, Result};
use ios_control_contracts::control::ControlCapability;
use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{
    BackendSelection, DeviceSessionStatus, DeviceSessionSummary, SessionPhase, SessionSubstate,
};
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use ios_control_plugin_runtime::RunningPlugin;

use crate::{request_plugin, select_source_id, PluginPaths, StartSessionRequest};

pub async fn start_session_actor(request: StartSessionRequest) -> Result<DeviceSessionStatus> {
    let mut capture = RunningPlugin::spawn(&request.plugin_paths.capture).await?;
    let capture_descriptor = capture.handshake().await?;
    let capture_backend = request_capture_backend(&mut capture).await?;
    let source_id = select_source_id(
        request.selected_source_id,
        &capture_sources_for_backend(&capture_backend),
        &capture_descriptor,
    )?;

    let control_backend = select_control_backend(&request.plugin_paths).await?;
    let control_plugin_id = if control_backend == "control.window-bridge" {
        "control.window-bridge"
    } else {
        "control.ble"
    };
    let plugin_health = if control_backend == "control.ble" {
        PluginHealth::Healthy
    } else {
        PluginHealth::Degraded
    };

    DeviceSessionStatus::new(
        DeviceSessionSummary {
            device_id: request.device_id,
            device_name: request.device_name,
            phase: SessionPhase::Streaming,
            plugin_health,
            capture_plugin: Some(capture_descriptor.plugin_id),
            control_plugin: Some(control_plugin_id.into()),
            grounding_plugin: request
                .plugin_paths
                .grounding
                .as_ref()
                .map(|_| "grounding.core".into()),
        },
        SessionSubstate::ControlReady,
        BackendSelection {
            capture_backend: capture_backend,
            control_backend,
        },
        None,
    )
    .map_err(|err| anyhow!(err))
    .map(|status| {
        let _ = source_id;
        status
    })
}

fn capture_sources_for_backend(capture_backend: &str) -> Vec<ios_control_contracts::capture::VideoSource> {
    vec![ios_control_contracts::capture::VideoSource {
        source_id: if capture_backend == "capture.window.helper" {
            "window-helper-1".into()
        } else {
            "direct-1".into()
        },
        display_name: capture_backend.into(),
        kind: if capture_backend == "capture.window.helper" {
            ios_control_contracts::capture::SourceKind::Window
        } else {
            ios_control_contracts::capture::SourceKind::DirectReceiver
        },
    }]
}

async fn request_capture_backend(capture: &mut RunningPlugin) -> Result<String> {
    match request_plugin(capture, &HostToPlugin::ProbeCapture).await? {
        PluginToHost::CaptureCapability { capability } => Ok(capability.backend_id),
        other => Err(anyhow!("unexpected capture capability response: {other:?}")),
    }
}

async fn request_control_capability_for_path(path: &std::path::Path) -> Result<ControlCapability> {
    let mut plugin = RunningPlugin::spawn(path).await?;
    plugin.handshake().await?;
    match request_plugin(&mut plugin, &HostToPlugin::ProbeControl).await? {
        PluginToHost::ControlCapability { capability } => {
            let _ = plugin.stop().await;
            Ok(capability)
        }
        other => Err(anyhow!("unexpected control capability response: {other:?}")),
    }
}

async fn select_control_backend(paths: &PluginPaths) -> Result<String> {
    let ble = request_control_capability_for_path(&paths.control_ble).await?;
    if ble.supported {
        return Ok("control.ble".into());
    }

    let fallback = request_control_capability_for_path(&paths.control_fallback).await?;
    if fallback.supported {
        return Ok("control.window-bridge".into());
    }

    Ok("control.window-bridge".into())
}
