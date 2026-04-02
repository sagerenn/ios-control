use crate::backend::ControlCapability;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxProbeResult {
    pub supported: bool,
    pub reason: Option<String>,
}

impl LinuxProbeResult {
    pub fn from_service_name(service_name: Option<&str>) -> Self {
        match service_name {
            Some("org.bluez") => Self {
                supported: true,
                reason: None,
            },
            _ => Self {
                supported: false,
                reason: Some("org.bluez not available".into()),
            },
        }
    }

    pub fn as_capability(&self) -> ControlCapability {
        ControlCapability {
            supported: self.supported,
            reason: self.reason.clone(),
        }
    }
}
