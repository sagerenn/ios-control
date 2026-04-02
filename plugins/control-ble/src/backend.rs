#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlCapability {
    pub supported: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlSessionPhase {
    Unavailable,
    ReadyToAdvertise,
    Advertising,
    Connected,
    Error,
}
