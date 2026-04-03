use anyhow::{bail, Result};
use ios_control_contracts::grounding::{GroundingPlan, PlanKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowBridgeCommand {
    pub args: Vec<String>,
}

pub fn command_for_plan(source_id: &str, plan: &GroundingPlan) -> Result<WindowBridgeCommand> {
    if plan.failure.is_some() {
        bail!("cannot execute failed grounding plan");
    }

    let args = match plan.kind {
        PlanKind::Pointer => vec!["--source".into(), source_id.into(), "--pointer-plan".into()],
        PlanKind::Keyboard => vec!["--source".into(), source_id.into(), "--keyboard-plan".into()],
        PlanKind::Hybrid => vec!["--source".into(), source_id.into(), "--hybrid-plan".into()],
    };

    Ok(WindowBridgeCommand { args })
}
