use std::path::Path;

use ios_control_contracts::capture::{CaptureCapability, VideoSource};
use ios_control_contracts::control::ControlCapability;
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use ios_control_plugin_runtime::RunningPlugin;

pub mod bluetooth;
pub mod known_devices;
pub mod mirror;

fn block_on_plugin<T>(
    plugin_path: &Path,
    op: impl FnOnce(
        &tokio::runtime::Runtime,
        &Path,
    ) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    op(&runtime, plugin_path)
}

pub fn probe_capture_capability(plugin_path: &Path) -> anyhow::Result<CaptureCapability> {
    block_on_plugin(plugin_path, |runtime, path| {
        runtime.block_on(async move {
            let mut plugin = RunningPlugin::spawn(path).await?;
            plugin.handshake().await?;
            plugin.send(&HostToPlugin::ProbeCapture).await?;
            let reply = plugin.read().await?;
            let _ = plugin.stop().await;
            match reply {
                PluginToHost::CaptureCapability { capability } => Ok(capability),
                other => anyhow::bail!("unexpected capture capability response: {other:?}"),
            }
        })
    })
}

pub fn list_capture_sources(plugin_path: &Path) -> anyhow::Result<Vec<VideoSource>> {
    block_on_plugin(plugin_path, |runtime, path| {
        runtime.block_on(async move {
            let mut plugin = RunningPlugin::spawn(path).await?;
            plugin.handshake().await?;
            plugin.send(&HostToPlugin::ListCaptureSources).await?;
            let reply = plugin.read().await?;
            let _ = plugin.stop().await;
            match reply {
                PluginToHost::CaptureSources { sources } => Ok(sources),
                other => anyhow::bail!("unexpected capture sources response: {other:?}"),
            }
        })
    })
}

pub fn probe_control_capability(plugin_path: &Path) -> anyhow::Result<ControlCapability> {
    block_on_plugin(plugin_path, |runtime, path| {
        runtime.block_on(async move {
            let mut plugin = RunningPlugin::spawn(path).await?;
            plugin.handshake().await?;
            plugin.send(&HostToPlugin::ProbeControl).await?;
            let reply = plugin.read().await?;
            let _ = plugin.stop().await;
            match reply {
                PluginToHost::ControlCapability { capability } => Ok(capability),
                other => anyhow::bail!("unexpected control capability response: {other:?}"),
            }
        })
    })
}
