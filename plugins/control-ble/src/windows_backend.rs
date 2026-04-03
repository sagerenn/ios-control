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
    let radio_present = std::env::var_os("IOS_CONTROL_BLE_RADIO_PRESENT").is_some();
    let peripheral_role_supported = match std::env::var("IOS_CONTROL_BLE_PERIPHERAL_ROLE") {
        Ok(value) => value == "1" || value.eq_ignore_ascii_case("true"),
        Err(_) => false,
    };
    WindowsProbeResult::from_runtime_checks(peripheral_role_supported, radio_present)
}
