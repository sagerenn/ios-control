use ios_control_contracts::grounding::GroundingFailure;

#[derive(Debug, Default)]
pub struct RecoveryController {
    retries_used: u8,
}

impl RecoveryController {
    pub fn next_action(&mut self, obvious_retry: bool) -> Result<bool, GroundingFailure> {
        if obvious_retry && self.retries_used == 0 {
            self.retries_used += 1;
            return Ok(true);
        }

        Err(GroundingFailure::RecoveryExhausted)
    }
}
