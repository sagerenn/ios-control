use ios_control_contracts::control::{
    ControlCapability as ContractControlCapability, ControlSessionPhase, ControlSetupChecklist,
    ControlTransportKind, ExecutionPhase, ExecutionSummary,
};
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};
use plugin_control_ble::backend::{ControlCapability, ControlSession, ControlSessionState};
use plugin_control_ble::helper_bridge::{run_execute, run_prepare, BleHelperExecution};
use plugin_control_ble::helper_config::{find_ble_helper, probe_ble_helper};
use plugin_control_ble::linux_backend::probe_linux_backend;
use plugin_control_ble::windows_backend::probe_windows_backend;
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

fn probe_control_capability() -> ControlCapability {
    let helper_capability = probe_ble_helper(find_ble_helper());
    if helper_capability.supported {
        return helper_capability;
    }

    let native_hint = if cfg!(target_os = "linux") {
        probe_linux_backend()
            .as_capability()
            .supported
            .then_some("native BLE transport detected; helper-backed execution is still required")
    } else if cfg!(target_os = "windows") {
        probe_windows_backend()
            .as_capability()
            .supported
            .then_some("native BLE transport detected; helper-backed execution is still required")
    } else {
        None
    };

    let reason = match (helper_capability.reason, native_hint) {
        (Some(helper_reason), Some(hint)) => Some(format!("{helper_reason}; {hint}")),
        (Some(helper_reason), None) => Some(helper_reason),
        (None, Some(hint)) => Some(hint.into()),
        (None, None) => Some(
            "configure IOS_CONTROL_BLE_HELPER with probe/prepare/execute support".into(),
        ),
    };

    ControlCapability {
        supported: false,
        reason,
    }
}

fn to_contract_capability(capability: &ControlCapability) -> ContractControlCapability {
    ContractControlCapability {
        supported: capability.supported,
        reason: capability.reason.clone(),
        transport: ControlTransportKind::BleHid,
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
        vec![
            "Enable Bluetooth".into(),
            "Pair the device when it appears".into(),
        ],
        vec!["BLE control path is ready for execution".into()],
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
    (phase, ControlSetupChecklist { items })
}

fn ble_helper_path_if_supported() -> Option<PathBuf> {
    let helper = find_ble_helper()?;
    let capability = probe_ble_helper(Some(helper.clone()));
    capability.supported.then_some(helper)
}

fn map_execution_phase(phase: &str) -> ExecutionPhase {
    match phase {
        "Pending" => ExecutionPhase::Pending,
        "Running" => ExecutionPhase::Running,
        "Succeeded" => ExecutionPhase::Succeeded,
        "Failed" => ExecutionPhase::Failed,
        _ => ExecutionPhase::Failed,
    }
}

fn summary_from_helper_execution(execution: BleHelperExecution) -> ExecutionSummary {
    let phase = map_execution_phase(&execution.phase);
    let failure_reason = if matches!(phase, ExecutionPhase::Failed) {
        execution
            .failure_reason
            .or_else(|| Some("ble helper reported failed execution".into()))
    } else {
        execution.failure_reason
    };
    ExecutionSummary {
        summary: execution.summary,
        phase,
        failure_reason,
    }
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
                let mut snapshot = build_session_from_capability(&capability);
                if let Some(helper) = ble_helper_path_if_supported() {
                    if let Err(err) = run_prepare(&helper) {
                        snapshot = ControlSession {
                            state: ControlSessionState::Error(err.to_string()),
                            checklist: vec![],
                            notes: vec!["ble helper prepare failed".into()],
                            pending_reports: 0,
                        };
                    }
                }
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
                            if let Some(helper) = ble_helper_path_if_supported() {
                                match run_execute(&helper, plan.kind.as_str()) {
                                    Ok(execution) => {
                                        summary = summary_from_helper_execution(execution);
                                    }
                                    Err(err) => {
                                        summary.summary = "ble helper execution failed".into();
                                        summary.phase = ExecutionPhase::Failed;
                                        summary.failure_reason = Some(err.to_string());
                                    }
                                }
                            } else {
                                summary.summary = "ble execution helper unavailable".into();
                                summary.failure_reason = Some(
                                    "configure IOS_CONTROL_BLE_HELPER with probe/prepare/execute support"
                                        .into(),
                                );
                                summary.phase = ExecutionPhase::Failed;
                            }
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
