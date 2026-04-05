#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlCapability {
    pub supported: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlSessionState {
    Unsupported,
    Ready,
    Advertising,
    Pairing,
    BondedIdle,
    ReconnectPending,
    Connected,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlSession {
    pub state: ControlSessionState,
    pub checklist: Vec<String>,
    pub notes: Vec<String>,
    pub pending_reports: usize,
}

impl ControlSession {
    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self {
            state: ControlSessionState::Unsupported,
            checklist: Vec::new(),
            notes: vec![reason.into()],
            pending_reports: 0,
        }
    }

    pub fn ready(checklist: Vec<String>, notes: Vec<String>) -> Self {
        Self {
            state: ControlSessionState::Ready,
            checklist,
            notes,
            pending_reports: 0,
        }
    }

    pub fn record_report_submission(&mut self, reports: usize) {
        self.pending_reports = self.pending_reports.saturating_add(reports);
    }
}
