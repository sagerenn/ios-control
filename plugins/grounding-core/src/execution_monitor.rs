use ios_control_contracts::grounding::GroundingFailure;

use crate::recovery_controller::RecoveryController;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionDecision {
    Applied,
    Retry,
    Failed(GroundingFailure),
}

pub struct ExecutionMonitor;

impl ExecutionMonitor {
    pub fn screen_changed(before: u64, after: u64) -> bool {
        before != after
    }

    pub fn evaluate(
        before: u64,
        after: u64,
        recovery: &mut RecoveryController,
    ) -> ExecutionDecision {
        if Self::screen_changed(before, after) {
            return ExecutionDecision::Applied;
        }

        match recovery.next_action(true) {
            Ok(true) => ExecutionDecision::Retry,
            Ok(false) => ExecutionDecision::Failed(GroundingFailure::ExecutionMismatch),
            Err(err) => ExecutionDecision::Failed(err),
        }
    }
}
