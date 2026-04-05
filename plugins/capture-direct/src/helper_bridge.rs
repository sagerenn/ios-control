use anyhow::Result;
use ios_control_contracts::capture::FrameHealth;
use ios_control_frame_transport::decode_base64_bytes;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HelperProbe {
    pub available: bool,
    pub supports_input_bridge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HelperFrameEvent {
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub rotation_degrees: u16,
    #[serde(default = "default_frame_health")]
    pub health: FrameHealth,
    pub rgba_base64: String,
}

fn default_frame_health() -> FrameHealth {
    FrameHealth::Healthy
}

impl HelperFrameEvent {
    pub fn decode_rgba(&self) -> Result<Vec<u8>> {
        decode_base64_bytes(&self.rgba_base64)
    }
}
