use std::path::Path;
use std::process::Stdio;

use anyhow::{anyhow, Result};
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginToHost};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

const PROTOCOL_VERSION: u32 = 2;

pub struct PluginRuntime;

impl PluginRuntime {
    pub fn new() -> Self {
        Self
    }

    pub async fn handshake(&self, plugin_path: &Path) -> Result<PluginDescriptor> {
        let mut child = Command::new(plugin_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("missing stdin"))?;
        let message = serde_json::to_string(&HostToPlugin::Handshake {
            protocol_version: PROTOCOL_VERSION,
        })?;
        stdin.write_all(message.as_bytes()).await?;
        stdin.write_all(b"\n").await?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("missing stdout"))?;
        let mut lines = BufReader::new(stdout).lines();
        let line = lines
            .next_line()
            .await?
            .ok_or_else(|| anyhow!("missing reply"))?;

        match serde_json::from_str::<PluginToHost>(&line)? {
            PluginToHost::HandshakeAck { descriptor } => {
                if descriptor.protocol_version != PROTOCOL_VERSION {
                    return Err(anyhow!(
                        "protocol version mismatch: expected {PROTOCOL_VERSION}, got {}",
                        descriptor.protocol_version
                    ));
                }
                Ok(descriptor)
            }
            other => Err(anyhow!("unexpected handshake response: {other:?}")),
        }
    }
}
