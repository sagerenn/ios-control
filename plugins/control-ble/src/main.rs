use ios_control_contracts::control::{
    ControlCapability, ControlSessionPhase, ControlSetupChecklist,
};
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};
use std::error::Error;
use std::io::{self, BufRead, Write};

const PROTOCOL_VERSION: u32 = 2;

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
    let mut handshaken = false;

    while let Some(line) = lines.next() {
        let line = line?;
        let request: HostToPlugin = serde_json::from_str(&line)?;
        match request {
            HostToPlugin::Handshake { .. } => {
                let reply = PluginToHost::HandshakeAck {
                    descriptor: PluginDescriptor {
                        plugin_id: "control.ble".into(),
                        protocol_version: PROTOCOL_VERSION,
                        kind: PluginKind::Control,
                        display_name: "Bluetooth Control".into(),
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
                    message: "handshake required for control plugin".into(),
                };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::ProbeControl => {
                let reply = PluginToHost::ControlCapability {
                    capability: ControlCapability {
                        supported: true,
                        reason: None,
                    },
                };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::PrepareControl => {
                let reply = PluginToHost::ControlSession {
                    phase: ControlSessionPhase::ReadyToAdvertise,
                    checklist: ControlSetupChecklist {
                        items: vec!["Enable Bluetooth".into(), "Pair the device".into()],
                    },
                };
                write_reply(&mut stdout, &reply)?;
            }
            _ => {
                let reply = PluginToHost::Error {
                    message: "unsupported request for control plugin".into(),
                };
                write_reply(&mut stdout, &reply)?;
            }
        }
    }

    Ok(())
}
