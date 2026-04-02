#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryEvent {
    pub session_id: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct TelemetryStore {
    events: Vec<TelemetryEvent>,
}

impl TelemetryStore {
    pub fn push(&mut self, event: TelemetryEvent) {
        self.events.push(event);
    }
}
