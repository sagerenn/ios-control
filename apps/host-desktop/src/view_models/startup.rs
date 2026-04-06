#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupReadiness {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupItem {
    pub label: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectReceiverViewModel {
    pub available: bool,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupViewModel {
    pub readiness: StartupReadiness,
    pub summary: String,
    pub items: Vec<StartupItem>,
    pub direct_receiver: DirectReceiverViewModel,
}

impl StartupViewModel {
    pub fn blocked(summary: impl Into<String>) -> Self {
        Self {
            readiness: StartupReadiness::Blocked,
            summary: summary.into(),
            items: Vec::new(),
            direct_receiver: DirectReceiverViewModel {
                available: false,
                status: "Blocked".into(),
                detail: "Direct receiver unavailable".into(),
            },
        }
    }
}
