use std::path::Path;
use std::process::Stdio;

use anyhow::{anyhow, Result};
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginToHost};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

const PROTOCOL_VERSION: u32 = 2;

pub struct PluginRuntime;

pub struct RunningPlugin {
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    stdout: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
}

impl RunningPlugin {
    pub async fn spawn(plugin_path: &Path) -> Result<Self> {
        let mut child = Command::new(plugin_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("missing stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("missing stdout"))?;
        let stdout = BufReader::new(stdout).lines();

        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }

    pub async fn send(&mut self, message: &HostToPlugin) -> Result<()> {
        let message = serde_json::to_string(message)?;
        self.send_raw(&message).await
    }

    pub async fn send_raw(&mut self, message: &str) -> Result<()> {
        self.stdin.write_all(message.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        Ok(())
    }

    pub async fn read(&mut self) -> Result<PluginToHost> {
        let line = self
            .stdout
            .next_line()
            .await?
            .ok_or_else(|| anyhow!("missing reply"))?;
        Ok(serde_json::from_str(&line)?)
    }

    pub async fn handshake(&mut self) -> Result<PluginDescriptor> {
        self.send(&HostToPlugin::Handshake {
            protocol_version: PROTOCOL_VERSION,
        })
        .await?;
        match self.read().await? {
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

    pub async fn stop(&mut self) -> Result<()> {
        self.send(&HostToPlugin::Stop).await?;
        match self.read().await? {
            PluginToHost::Ack => {}
            other => return Err(anyhow!("unexpected stop response: {other:?}")),
        }
        let status = self.child.wait().await?;
        if !status.success() {
            return Err(anyhow!("plugin exited with status {status}"));
        }
        Ok(())
    }
}

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
