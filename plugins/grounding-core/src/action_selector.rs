use ios_control_contracts::grounding::{GroundingFailure, PlanKind};

use crate::focus_tracker::FocusTracker;

#[derive(Debug, Clone, PartialEq)]
pub struct SelectedPlan {
    pub kind: PlanKind,
}

#[derive(Debug, Default)]
pub struct ActionSelector;

impl ActionSelector {
    pub fn choose_plan(
        &self,
        pointer_possible: bool,
        focus: &FocusTracker,
        pointer_uncertainty: f32,
    ) -> Result<SelectedPlan, GroundingFailure> {
        if focus.prefers_keyboard() {
            return Ok(SelectedPlan {
                kind: PlanKind::Keyboard,
            });
        }

        if pointer_possible && pointer_uncertainty < 80.0 {
            return Ok(SelectedPlan {
                kind: PlanKind::Pointer,
            });
        }

        Err(GroundingFailure::GeometryUncertain)
    }
}
