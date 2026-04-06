use crate::backend::HostCapability;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperState {
    pub phase: String,
    pub checklist: Vec<String>,
    pub notes: Vec<String>,
    pub paired_device_id: Option<String>,
    pub paired_device_name: Option<String>,
    pub bonded: bool,
    pub execute_ready: bool,
}

impl Default for HelperState {
    fn default() -> Self {
        Self {
            phase: "Advertising".into(),
            checklist: Vec::new(),
            notes: Vec::new(),
            paired_device_id: None,
            paired_device_name: None,
            bonded: false,
            execute_ready: false,
        }
    }
}

pub fn helper_state_from_capability(capability: &HostCapability, bonded: bool) -> HelperState {
    if !capability.supported {
        return HelperState {
            phase: "Unavailable".into(),
            checklist: vec!["Use fallback control or install supported Bluetooth support".into()],
            notes: vec![capability
                .reason
                .clone()
                .unwrap_or_else(|| "BLE unavailable".into())],
            paired_device_id: None,
            paired_device_name: None,
            bonded: false,
            execute_ready: false,
        };
    }

    let phase = if bonded {
        "BondedIdle"
    } else {
        "ReadyToAdvertise"
    };
    let checklist = if bonded {
        vec!["Reconnect the paired device".into()]
    } else {
        vec!["Enable Bluetooth".into(), "Pair the device when it appears".into()]
    };

    HelperState {
        phase: phase.into(),
        checklist,
        notes: vec![format!("{} backend available", capability.backend)],
        paired_device_id: None,
        paired_device_name: None,
        bonded,
        execute_ready: false,
    }
}
