use anyhow::Result;
use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{DeviceSessionSummary, SessionPhase};

#[derive(Debug, Clone)]
pub struct RequestedPlugins {
    pub capture: String,
    pub control: String,
    pub grounding: Option<String>,
}

pub struct CaptureRouting {
    pub selected_source: Option<String>,
}

#[derive(Debug, Default)]
pub struct SessionOrchestrator;

impl SessionOrchestrator {
    pub async fn start_session(
        &self,
        device_id: &str,
        requested: RequestedPlugins,
    ) -> Result<DeviceSessionSummary> {
        Ok(DeviceSessionSummary {
            device_id: device_id.into(),
            device_name: device_id.into(),
            phase: SessionPhase::Connecting,
            plugin_health: PluginHealth::Unknown,
            capture_plugin: Some(requested.capture),
            control_plugin: Some(requested.control),
            grounding_plugin: requested.grounding,
        })
    }
}
