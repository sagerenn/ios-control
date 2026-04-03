use crate::backend::ControlCapability;
use std::path::Path;

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

    pub fn from_runtime_checks(peripheral_role_supported: bool, radio_present: bool) -> Self {
        if !radio_present {
            return Self {
                supported: false,
                reason: Some("bluetooth radio not detected".into()),
            };
        }
        if !peripheral_role_supported {
            return Self {
                supported: false,
                reason: Some("bluetooth peripheral role not supported".into()),
            };
        }
        Self {
            supported: true,
            reason: None,
        }
    }

    pub fn as_capability(&self) -> ControlCapability {
        ControlCapability {
            supported: self.supported,
            reason: self.reason.clone(),
        }
    }
}

pub fn probe_windows_backend() -> WindowsProbeResult {
    if !cfg!(target_os = "windows") {
        return WindowsProbeResult::from_runtime_checks(false, false);
    }

    let radio_present = Path::new(r"C:\Windows\System32\drivers\BTHport.sys").exists()
        || Path::new(r"C:\Windows\System32\drivers\BthLEEnum.sys").exists();
    let peripheral_role_supported =
        Path::new(r"C:\Windows\System32\drivers\BthLEEnum.sys").exists();
    WindowsProbeResult::from_runtime_checks(peripheral_role_supported, radio_present)
}
