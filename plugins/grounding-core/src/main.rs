use ios_control_contracts::grounding::{GroundingPlan, PlanKind};
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

    while let Some(line) = lines.next() {
        let line = line?;
        let request: HostToPlugin = serde_json::from_str(&line)?;
        match request {
            HostToPlugin::Handshake { .. } => {
                let reply = PluginToHost::HandshakeAck {
                    descriptor: PluginDescriptor {
                        plugin_id: "grounding.core".into(),
                        protocol_version: PROTOCOL_VERSION,
                        kind: PluginKind::Grounding,
                        display_name: "Grounding Core".into(),
                    },
                };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::PlanGrounding { request } => {
                let kind = if request.keyboard_preferred {
                    PlanKind::Keyboard
                } else {
                    PlanKind::Pointer
                };
                let reply = PluginToHost::GroundingPlan {
                    plan: GroundingPlan {
                        kind,
                        failure: None,
                        summary: format!("selected {} plan", kind.as_str()),
                    },
                };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::Stop => {
                write_reply(&mut stdout, &PluginToHost::Ack)?;
                break;
            }
            _ => {
                let reply = PluginToHost::Error {
                    message: "unsupported request for grounding plugin".into(),
                };
                write_reply(&mut stdout, &reply)?;
            }
        }
    }

    Ok(())
}
