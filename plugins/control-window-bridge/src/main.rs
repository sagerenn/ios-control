use ios_control_contracts::control::{
    ControlCapability, ControlSessionPhase, ControlSetupChecklist, ControlTransportKind,
    ExecutionPhase, ExecutionSummary,
};
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};
use plugin_control_window_bridge::backend::command_for_plan;
use std::error::Error;
use std::io::{self, BufRead, Write};

const PROTOCOL_VERSION: u32 = 3;

fn write_reply(stdout: &mut impl Write, reply: &PluginToHost) -> Result<(), Box<dyn Error>> {
    let payload = serde_json::to_string(reply)?;
    stdout.write_all(payload.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn control_capability() -> ControlCapability {
    ControlCapability {
        supported: std::env::var_os("IOS_CONTROL_WINDOW_INPUT_HELPER").is_some(),
        reason: std::env::var_os("IOS_CONTROL_WINDOW_INPUT_HELPER")
            .is_none()
            .then_some("IOS_CONTROL_WINDOW_INPUT_HELPER not configured".into()),
        transport: ControlTransportKind::WindowInputBridge,
    }
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
                        plugin_id: "control.window-bridge".into(),
                        protocol_version: PROTOCOL_VERSION,
                        kind: PluginKind::Control,
                        display_name: "Window Input Bridge".into(),
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
                write_reply(
                    &mut stdout,
                    &PluginToHost::Error {
                        message: "handshake required for control plugin".into(),
                    },
                )?;
            }
            HostToPlugin::ProbeControl => {
                write_reply(
                    &mut stdout,
                    &PluginToHost::ControlCapability {
                        capability: control_capability(),
                    },
                )?;
            }
            HostToPlugin::PrepareControl => {
                let reply = PluginToHost::ControlSession {
                    phase: ControlSessionPhase::ReadyToAdvertise,
                    checklist: ControlSetupChecklist {
                        items: vec![
                            "Configure IOS_CONTROL_WINDOW_INPUT_HELPER".into(),
                            "Keep the mirrored window visible and focused".into(),
                        ],
                    },
                };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::ExecutePlan { plan } => {
                let summary = match command_for_plan("window-helper-1", &plan) {
                    Ok(command) => ExecutionSummary {
                        summary: format!("launched window bridge helper {:?}", command.args),
                        phase: ExecutionPhase::Succeeded,
                        failure_reason: None,
                    },
                    Err(err) => ExecutionSummary {
                        summary: "window bridge execution failed".into(),
                        phase: ExecutionPhase::Failed,
                        failure_reason: Some(err.to_string()),
                    },
                };
                write_reply(&mut stdout, &PluginToHost::ExecutionSummary { summary })?;
            }
            _ => {
                write_reply(
                    &mut stdout,
                    &PluginToHost::Error {
                        message: "unsupported request for control plugin".into(),
                    },
                )?;
            }
        }
    }

    Ok(())
}
