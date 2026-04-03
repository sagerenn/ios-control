use ios_control_contracts::grounding::GroundingFailure;

use crate::recovery_controller::RecoveryController;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionDecision {
    ObservedChange,
    Retry,
    Failed(GroundingFailure),
}

pub struct ExecutionMonitor;

impl ExecutionMonitor {
    pub fn frame_advanced(before: u64, after: u64) -> bool {
        before != after
    }

    pub fn evaluate(
        before: u64,
        after: u64,
        recovery: &mut RecoveryController,
    ) -> ExecutionDecision {
        if Self::frame_advanced(before, after) {
            return ExecutionDecision::ObservedChange;
        }

        match recovery.next_action(true) {
            Ok(true) => ExecutionDecision::Retry,
            Ok(false) => ExecutionDecision::Failed(GroundingFailure::ExecutionMismatch),
            Err(err) => ExecutionDecision::Failed(err),
        }
    }
}
