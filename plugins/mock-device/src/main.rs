use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut lines = BufReader::new(io::stdin()).lines();
    let mut stdout = io::stdout();

    if let Some(line) = lines.next_line().await? {
        let request: HostToPlugin = serde_json::from_str(&line)?;
        if let HostToPlugin::Handshake { .. } = request {
            let reply = PluginToHost::HandshakeAck {
                descriptor: PluginDescriptor {
                    plugin_id: "mock.device".into(),
                    protocol_version: 2,
                    kind: PluginKind::Control,
                    display_name: "Mock Device".into(),
                },
            };
            stdout.write_all(serde_json::to_string(&reply)?.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
        }
    }

    Ok(())
}
