use serde::{Deserialize, Serialize};

use crate::plugin::PluginHealth;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionPhase {
    Disconnected,
    Connecting,
    Streaming,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSessionSummary {
    pub device_id: String,
    pub device_name: String,
    pub phase: SessionPhase,
    pub plugin_health: PluginHealth,
    pub capture_plugin: Option<String>,
    pub control_plugin: Option<String>,
    pub grounding_plugin: Option<String>,
}

impl DeviceSessionSummary {
    pub fn new(device_id: String, device_name: String) -> Self {
        Self {
            device_id,
            device_name,
            phase: SessionPhase::Disconnected,
            plugin_health: PluginHealth::Unknown,
            capture_plugin: None,
            control_plugin: None,
            grounding_plugin: None,
        }
    }
}
