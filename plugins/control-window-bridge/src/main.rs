use ios_control_contracts::control::{
    ControlCapability, ControlSessionPhase, ControlSetupChecklist, ControlTransportKind,
    ExecutionPhase, ExecutionSummary,
};
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};
use plugin_control_window_bridge::backend::command_for_plan;
use plugin_control_window_bridge::helper_launcher::{
    find_helper, helper_available, helper_is_executable, launch_helper, run_embedded_helper_mode,
    should_run_embedded_helper_mode,
};
use std::error::Error;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

const PROTOCOL_VERSION: u32 = 3;

fn write_reply(stdout: &mut impl Write, reply: &PluginToHost) -> Result<(), Box<dyn Error>> {
    let payload = serde_json::to_string(reply)?;
    stdout.write_all(payload.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn control_capability() -> ControlCapability {
    let configured = std::env::var_os("IOS_CONTROL_WINDOW_INPUT_HELPER");
    let configured_path = configured.clone().map(PathBuf::from);
    let discovered = find_helper();
    let supported = helper_available(discovered.clone());
    ControlCapability {
        supported,
        reason: if supported {
            None
        } else if configured.is_none() {
            Some("IOS_CONTROL_WINDOW_INPUT_HELPER not configured".into())
        } else if discovered.is_none() {
            Some("IOS_CONTROL_WINDOW_INPUT_HELPER does not point to a file".into())
        } else if !configured_path
            .as_ref()
            .is_some_and(|path| helper_is_executable(path))
        {
            Some("IOS_CONTROL_WINDOW_INPUT_HELPER is not executable".into())
        } else {
            Some("IOS_CONTROL_WINDOW_INPUT_HELPER is not usable".into())
        },
        transport: ControlTransportKind::WindowInputBridge,
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if should_run_embedded_helper_mode(&args) {
        run_embedded_helper_mode(&args)?;
        return Ok(());
    }

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
                let capability = control_capability();
                let reply = PluginToHost::ControlSession {
                    phase: if capability.supported {
                        ControlSessionPhase::ReadyToAdvertise
                    } else {
                        ControlSessionPhase::Unavailable
                    },
                    checklist: ControlSetupChecklist {
                        items: if capability.supported {
                            vec![
                                "Configure IOS_CONTROL_WINDOW_INPUT_HELPER".into(),
                                "Keep the mirrored window visible and focused".into(),
                            ]
                        } else {
                            vec![capability
                                .reason
                                .unwrap_or_else(|| "window bridge helper unavailable".into())]
                        },
                    },
                };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::ExecutePlan { plan } => {
                let summary = match (find_helper(), command_for_plan("window-helper-1", &plan)) {
                    (Some(helper), Ok(command)) => match launch_helper(helper, &command.args) {
                        Ok(status) if status.success() => ExecutionSummary {
                            summary: "window bridge helper executed".into(),
                            phase: ExecutionPhase::Succeeded,
                            failure_reason: None,
                        },
                        Ok(_) => ExecutionSummary {
                            summary: "window bridge execution failed".into(),
                            phase: ExecutionPhase::Failed,
                            failure_reason: Some("helper returned non-zero exit status".into()),
                        },
                        Err(err) => ExecutionSummary {
                            summary: "window bridge execution failed".into(),
                            phase: ExecutionPhase::Failed,
                            failure_reason: Some(err.to_string()),
                        },
                    },
                    (_, Err(err)) => ExecutionSummary {
                        summary: "window bridge execution failed".into(),
                        phase: ExecutionPhase::Failed,
                        failure_reason: Some(err.to_string()),
                    },
                    (None, Ok(_)) => ExecutionSummary {
                        summary: "window bridge execution failed".into(),
                        phase: ExecutionPhase::Failed,
                        failure_reason: if std::env::var_os("IOS_CONTROL_WINDOW_INPUT_HELPER")
                            .is_some()
                        {
                            Some("IOS_CONTROL_WINDOW_INPUT_HELPER does not point to a file".into())
                        } else {
                            Some("IOS_CONTROL_WINDOW_INPUT_HELPER not configured".into())
                        },
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
