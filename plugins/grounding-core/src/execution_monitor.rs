pub struct ExecutionMonitor;

impl ExecutionMonitor {
    pub fn screen_changed(before: u64, after: u64) -> bool {
        before != after
    }
}
