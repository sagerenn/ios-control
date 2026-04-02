use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginDescriptor {
    pub plugin_id: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HostToPlugin {
    Handshake { protocol_version: u32 },
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginToHost {
    HandshakeAck { descriptor: PluginDescriptor },
}
