use ios_control_contracts::control::{
    ControlCapability as ContractControlCapability, ControlSessionPhase, ControlSetupChecklist,
    ControlTransportKind, ExecutionPhase, ExecutionSummary,
};
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};
use plugin_control_ble::backend::{ControlCapability, ControlSession, ControlSessionState};
use plugin_control_ble::helper_bridge::{
    run_control_input, run_execute, run_prepare, run_status, run_stop, BleHelperExecution,
    BleHelperPrepare, BleHelperStatus,
};
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

    let native_hint =
        if cfg!(target_os = "linux") {
            probe_linux_backend().as_capability().supported.then_some(
                "native BLE transport detected; helper-backed execution is still required",
            )
        } else if cfg!(target_os = "windows") {
            probe_windows_backend().as_capability().supported.then_some(
                "native BLE transport detected; helper-backed execution is still required",
            )
        } else {
            None
        };

    let reason = match (helper_capability.reason, native_hint) {
        (Some(helper_reason), Some(hint)) => Some(format!("{helper_reason}; {hint}")),
        (Some(helper_reason), None) => Some(helper_reason),
        (None, Some(hint)) => Some(hint.into()),
        (None, None) => {
            Some("configure IOS_CONTROL_BLE_HELPER with probe/prepare/execute support".into())
        }
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

fn helper_prepare_to_session(prepare: BleHelperPrepare) -> ControlSession {
    helper_runtime_to_session(prepare.phase, prepare.checklist, prepare.notes)
}

fn helper_status_to_session(status: BleHelperStatus) -> ControlSession {
    helper_runtime_to_session(status.phase, status.checklist, status.notes)
}

fn helper_runtime_to_session(
    phase: String,
    checklist: Vec<String>,
    notes: Vec<String>,
) -> ControlSession {
    let state = match phase.as_str() {
        "Advertising" => ControlSessionState::Advertising,
        "Pairing" => ControlSessionState::Pairing,
        "BondedIdle" => ControlSessionState::BondedIdle,
        "ReconnectPending" => ControlSessionState::ReconnectPending,
        "Connected" => ControlSessionState::Connected,
        "ReadyToAdvertise" => ControlSessionState::Ready,
        "Unavailable" => ControlSessionState::Unsupported,
        "Error" => ControlSessionState::Error("ble helper prepare reported error".into()),
        _ => ControlSessionState::Ready,
    };

    ControlSession {
        state,
        checklist,
        notes,
        pending_reports: 0,
    }
}

fn session_to_contract(session: &ControlSession) -> (ControlSessionPhase, ControlSetupChecklist) {
    let phase = match session.state {
        ControlSessionState::Unsupported => ControlSessionPhase::Unavailable,
        ControlSessionState::Ready => ControlSessionPhase::ReadyToAdvertise,
        ControlSessionState::Advertising => ControlSessionPhase::Advertising,
        ControlSessionState::Pairing => ControlSessionPhase::Pairing,
        ControlSessionState::BondedIdle => ControlSessionPhase::BondedIdle,
        ControlSessionState::ReconnectPending => ControlSessionPhase::ReconnectPending,
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
        observed_change: Some(execution.observed_change),
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
                if session.is_some() {
                    if let Some(helper) = find_ble_helper() {
                        let _ = run_stop(&helper);
                    }
                }
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
                    match run_prepare(&helper) {
                        Ok(prepare) => {
                            snapshot = helper_prepare_to_session(prepare);
                        }
                        Err(err) => {
                            snapshot = ControlSession {
                                state: ControlSessionState::Error(err.to_string()),
                                checklist: vec![],
                                notes: vec!["ble helper prepare failed".into()],
                                pending_reports: 0,
                            };
                        }
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
                    observed_change: None,
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
                                        summary.observed_change = None;
                                        summary.failure_reason = Some(err.to_string());
                                    }
                                }
                            } else {
                                summary.summary = "ble execution helper unavailable".into();
                                summary.observed_change = None;
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
            HostToPlugin::ForwardControlInput { event } => {
                let mut summary = ExecutionSummary {
                    summary: "no control session prepared".into(),
                    phase: ExecutionPhase::Failed,
                    observed_change: None,
                    failure_reason: Some("call PrepareControl before ForwardControlInput".into()),
                };

                if session.is_none() {
                    let capability = probe_control_capability();
                    session = Some(build_session_from_capability(&capability));
                }
                if session.as_ref().is_some_and(|active| {
                    matches!(
                        active.state,
                        ControlSessionState::Error(_) | ControlSessionState::Unsupported
                    )
                }) {
                    if let Some(helper) = ble_helper_path_if_supported() {
                        if let Ok(status) = run_status(&helper) {
                            session = Some(helper_status_to_session(status));
                        }
                    }
                }

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
                                match run_control_input(&helper, event) {
                                    Ok(execution) => {
                                        summary = execution;
                                    }
                                    Err(err) => {
                                        summary.summary = "ble helper live input failed".into();
                                        summary.phase = ExecutionPhase::Failed;
                                        summary.observed_change = None;
                                        summary.failure_reason = Some(err.to_string());
                                    }
                                }
                            } else {
                                summary.summary = "ble execution helper unavailable".into();
                                summary.observed_change = None;
                                summary.failure_reason = Some(
                                    "configure IOS_CONTROL_BLE_HELPER with live input support"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_status_connected_maps_to_connected_session() {
        let session = helper_status_to_session(BleHelperStatus {
            phase: "Connected".into(),
            checklist: vec!["Enable Bluetooth".into()],
            notes: vec!["BLE HID keyboard client subscribed".into()],
            paired_device_id: Some("ble-hid-client".into()),
            paired_device_name: Some("BLE HID client".into()),
            bonded: true,
            execute_ready: true,
        });

        assert_eq!(session.state, ControlSessionState::Connected);
        assert_eq!(session.checklist, vec!["Enable Bluetooth"]);
        assert_eq!(
            session.notes,
            vec!["BLE HID keyboard client subscribed".to_string()]
        );
    }
}
