#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlCapability {
    pub supported: bool,
    pub reason: Option<String>,
}
