use ios_control_contracts::control::{
    ControlCapability as ContractControlCapability, ControlSessionPhase,
    ControlSetupChecklist, ExecutionPhase, ExecutionSummary,
};
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};
use plugin_control_ble::backend::{ControlCapability, ControlSession, ControlSessionState};
use plugin_control_ble::linux_backend::probe_linux_backend;
use plugin_control_ble::windows_backend::probe_windows_backend;
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

fn probe_control_capability() -> ControlCapability {
    if cfg!(target_os = "linux") {
        return probe_linux_backend().as_capability();
    }
    if cfg!(target_os = "windows") {
        return probe_windows_backend().as_capability();
    }
    ControlCapability {
        supported: false,
        reason: Some("unsupported host os for ble control".into()),
    }
}

fn to_contract_capability(capability: &ControlCapability) -> ContractControlCapability {
    ContractControlCapability {
        supported: capability.supported,
        reason: capability.reason.clone(),
    }
}

fn build_session_from_capability(capability: &ControlCapability) -> ControlSession {
    if !capability.supported {
        return ControlSession::unsupported(
            capability
                .reason
                .clone()
                .unwrap_or_else(|| "ble control not supported".into()),
        );
    }
    ControlSession::ready(
        vec!["Enable Bluetooth".into(), "Pair the device when it appears".into()],
        vec![
            "BLE advertising/connect not implemented yet".into(),
            "HID reports will be generated but not transmitted".into(),
        ],
    )
}

fn session_to_contract(session: &ControlSession) -> (ControlSessionPhase, ControlSetupChecklist) {
    let phase = match session.state {
        ControlSessionState::Unsupported => ControlSessionPhase::Unavailable,
        ControlSessionState::Ready => ControlSessionPhase::ReadyToAdvertise,
        ControlSessionState::Advertising => ControlSessionPhase::Advertising,
        ControlSessionState::Connected => ControlSessionPhase::Connected,
        ControlSessionState::Error(_) => ControlSessionPhase::Error,
    };
    let mut items = session.checklist.clone();
    items.extend(session.notes.clone());
    (
        phase,
        ControlSetupChecklist {
            items,
        },
    )
}

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout());
    let mut lines = stdin.lock().lines();
    let mut handshaken = false;
    let mut session: Option<ControlSession> = None;

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
                let capability = probe_control_capability();
                let reply = PluginToHost::ControlCapability {
                    capability: to_contract_capability(&capability),
                };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::PrepareControl => {
                let capability = probe_control_capability();
                let snapshot = build_session_from_capability(&capability);
                let (phase, checklist) = session_to_contract(&snapshot);
                session = Some(snapshot);
                let reply = PluginToHost::ControlSession { phase, checklist };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::ExecutePlan { plan } => {
                let mut summary = ExecutionSummary {
                    summary: "no control session prepared".into(),
                    phase: ExecutionPhase::Failed,
                    failure_reason: Some("call PrepareControl before ExecutePlan".into()),
                };

                if let Some(active) = session.as_mut() {
                    match &active.state {
                        ControlSessionState::Unsupported => {
                            summary.summary = "ble control unsupported on this host".into();
                            summary.failure_reason =
                                Some("ble control unsupported on this host".into());
                        }
                        ControlSessionState::Error(message) => {
                            summary.summary = "ble control session error".into();
                            summary.failure_reason = Some(message.clone());
                        }
                        _ => {
                            summary.summary = format!(
                                "execution payload for plan kind {} not implemented",
                                plan.kind.as_str()
                            );
                            summary.failure_reason =
                                Some("control execution requires a concrete action payload".into());
                            summary.phase = ExecutionPhase::Failed;
                        }
                    }
                }

                let reply = PluginToHost::ExecutionSummary { summary };
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
