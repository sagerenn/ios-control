#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryEvent {
    pub session_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct TelemetryStore {
    events: Vec<TelemetryEvent>,
}

impl TelemetryStore {
    pub fn push(&mut self, event: TelemetryEvent) {
        self.events.push(event);
    }

    pub fn for_session(&self, session_id: &str) -> Vec<TelemetryEvent> {
        self.events
            .iter()
            .filter(|event| event.session_id == session_id)
            .cloned()
            .collect()
    }

    pub fn events(&self) -> &[TelemetryEvent] {
        &self.events
    }
}
