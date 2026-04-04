use anyhow::Result;
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
    pub fill_byte: u8,
    #[serde(default)]
    pub rgba_base64: String,
}

impl HelperFrameEvent {
    pub fn decode_rgba(&self) -> Result<Vec<u8>> {
        decode_base64_bytes(&self.rgba_base64)
    }
}
