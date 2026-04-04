use anyhow::{anyhow, Result};
use ios_control_contracts::session::{
    BackendSelection, DeviceSessionStatus, DeviceSessionSummary, SessionPhase, SessionSubstate,
};

use crate::{ActiveSessionState, SessionOrchestrator, StartSessionRequest};

pub async fn start_session_actor(
    orchestrator: &mut SessionOrchestrator,
    request: StartSessionRequest,
) -> Result<ActiveSessionState> {
    orchestrator.start_session_with_plugins(request).await
}

pub fn status_snapshot(active: &ActiveSessionState) -> Result<DeviceSessionStatus> {
    let summary = active.summary.clone();
    let substate = substate_for_phase(summary.phase);
    DeviceSessionStatus::new(
        summary.clone(),
        substate,
        BackendSelection {
            capture_backend: capture_backend_for_summary(&summary),
            control_backend: control_backend_for_summary(&summary),
        },
        operator_action_for_substate(substate, &active.diagnostics.control_summary),
    )
    .map_err(|error| anyhow!(error))
}

fn substate_for_phase(phase: SessionPhase) -> SessionSubstate {
    match phase {
        SessionPhase::Disconnected => SessionSubstate::Stopped,
        SessionPhase::Connecting => SessionSubstate::StartingControl,
        SessionPhase::Streaming => SessionSubstate::ControlReady,
        SessionPhase::Degraded => SessionSubstate::DegradedControl,
    }
}

fn operator_action_for_substate(
    substate: SessionSubstate,
    control_summary: &str,
) -> Option<String> {
    match substate {
        SessionSubstate::OperatorActionRequired => Some(control_summary.to_string()),
        _ => None,
    }
}

fn capture_backend_for_summary(summary: &DeviceSessionSummary) -> String {
    match summary.capture_plugin.as_deref() {
        Some("capture.window") => "capture.window.helper".into(),
        Some("capture.direct") => "capture.direct".into(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn control_backend_for_summary(summary: &DeviceSessionSummary) -> String {
    match summary.control_plugin.as_deref() {
        Some("control.window-bridge") => "control.window-bridge".into(),
        Some("control.ble") => "control.ble".into(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}
