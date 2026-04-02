use crate::backend::ControlCapability;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsProbeResult {
    pub supported: bool,
    pub reason: Option<String>,
}

impl WindowsProbeResult {
    pub fn from_peripheral_role(supported: bool) -> Self {
        if supported {
            Self {
                supported: true,
                reason: None,
            }
        } else {
            Self {
                supported: false,
                reason: Some("bluetooth peripheral role not supported".into()),
            }
        }
    }

    pub fn as_capability(&self) -> ControlCapability {
        ControlCapability {
            supported: self.supported,
            reason: self.reason.clone(),
        }
    }
}
