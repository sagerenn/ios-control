use ios_control_contracts::grounding::{GroundingPlan, PlanKind};
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};
use std::error::Error;
use std::io::{self, BufRead, Write};

use plugin_grounding_core::action_selector::ActionSelector;
use plugin_grounding_core::focus_tracker::FocusTracker;

const PROTOCOL_VERSION: u32 = 3;

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
                        plugin_id: "grounding.core".into(),
                        protocol_version: PROTOCOL_VERSION,
                        kind: PluginKind::Grounding,
                        display_name: "Grounding Core".into(),
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
                    message: "handshake required for grounding plugin".into(),
                };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::PlanGrounding { request } => {
                let selector = ActionSelector::default();
                let focus = FocusTracker {
                    focus_confidence: request.focus_confidence,
                    keyboard_friendly: request.keyboard_preferred,
                };
                let pointer_possible = request.target.visual_region.is_some();
                let selection =
                    selector.choose_plan(pointer_possible, &focus, request.uncertainty_radius);
                let (kind, failure) = match selection {
                    Ok(plan) => (plan.kind, None),
                    Err(err) => (PlanKind::Hybrid, Some(err)),
                };
                let summary = match failure {
                    Some(reason) => format!("grounding failed: {}", reason.as_str()),
                    None => format!("selected {} plan", kind.as_str()),
                };
                let reply = PluginToHost::GroundingPlan {
                    plan: GroundingPlan {
                        kind,
                        failure,
                        summary,
                    },
                };
                write_reply(&mut stdout, &reply)?;
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
