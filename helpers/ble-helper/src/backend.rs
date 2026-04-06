#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCapability {
    pub supported: bool,
    pub backend: &'static str,
    pub reason: Option<String>,
}

impl HostCapability {
    pub fn supported(backend: &'static str) -> Self {
        Self {
            supported: true,
            backend,
            reason: None,
        }
    }

    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self {
            supported: false,
            backend: "unknown",
            reason: Some(reason.into()),
        }
    }
}
