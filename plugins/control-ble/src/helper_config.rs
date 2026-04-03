use crate::backend::ControlCapability;
use crate::helper_bridge::run_probe;
use std::path::PathBuf;

pub fn find_ble_helper() -> Option<PathBuf> {
    std::env::var_os("IOS_CONTROL_BLE_HELPER")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

pub fn probe_ble_helper(helper: Option<PathBuf>) -> ControlCapability {
    match helper {
        Some(path) => match run_probe(&path) {
            Ok(probe) if probe.supported && probe.supports_prepare && probe.supports_execute => {
                ControlCapability {
                    supported: true,
                    reason: None,
                }
            }
            Ok(_) => ControlCapability {
                supported: false,
                reason: Some("ble helper missing prepare/execute support".into()),
            },
            Err(err) => ControlCapability {
                supported: false,
                reason: Some(format!("ble helper probe failed: {err}")),
            },
        },
        None => ControlCapability {
            supported: false,
            reason: Some("IOS_CONTROL_BLE_HELPER not configured".into()),
        },
    }
}
