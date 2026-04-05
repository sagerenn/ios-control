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
pub struct StartupViewModel {
    pub readiness: StartupReadiness,
    pub summary: String,
    pub items: Vec<StartupItem>,
}

impl StartupViewModel {
    pub fn blocked(summary: impl Into<String>) -> Self {
        Self {
            readiness: StartupReadiness::Blocked,
            summary: summary.into(),
            items: Vec::new(),
        }
    }
}
