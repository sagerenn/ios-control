use ios_control_contracts::capture::{FrameHealth, SourceKind, VideoFrameDescriptor};
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};
use std::error::Error;
use std::io::{self, BufRead, Write};

const PROTOCOL_VERSION: u32 = 2;
const SOURCE_ID: &str = "direct-1";

fn write_reply(stdout: &mut impl Write, reply: &PluginToHost) -> Result<(), Box<dyn Error>> {
    let payload = serde_json::to_string(reply)?;
    stdout.write_all(payload.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout());
    let mut lines = stdin.lock().lines();
    let mut frame_index: u64 = 0;
    let mut handshaken = false;

    while let Some(line) = lines.next() {
        let line = line?;
        let request: HostToPlugin = serde_json::from_str(&line)?;
        match request {
            HostToPlugin::Handshake { .. } => {
                let reply = PluginToHost::HandshakeAck {
                    descriptor: PluginDescriptor {
                        plugin_id: "capture.direct".into(),
                        protocol_version: PROTOCOL_VERSION,
                        kind: PluginKind::Capture,
                        display_name: "Direct Receiver".into(),
                    },
                };
                handshaken = true;
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::Stop => {
                write_reply(&mut stdout, &PluginToHost::Ack)?;
                break;
            }
            _ if !handshaken => {
                let reply = PluginToHost::Error {
                    message: "handshake required for capture-direct plugin".into(),
                };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::StartDirectCapture => {
                frame_index += 1;
                let reply = PluginToHost::CaptureFrame {
                    frame: VideoFrameDescriptor {
                        source_id: SOURCE_ID.into(),
                        source_kind: SourceKind::DirectReceiver,
                        width: 1920,
                        height: 1080,
                        rotation_degrees: 0,
                        frame_index,
                        health: FrameHealth::Healthy,
                    },
                };
                write_reply(&mut stdout, &reply)?;
            }
            _ => {
                let reply = PluginToHost::Error {
                    message: "unsupported request for capture-direct plugin".into(),
                };
                write_reply(&mut stdout, &reply)?;
            }
        }
    }

    Ok(())
}
