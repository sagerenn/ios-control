use crate::backend::ControlCapability;
use std::path::PathBuf;

pub fn find_ble_helper() -> Option<PathBuf> {
    std::env::var_os("IOS_CONTROL_BLE_HELPER")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

pub fn probe_ble_helper(helper: Option<PathBuf>) -> ControlCapability {
    match helper {
        Some(_) => ControlCapability {
            supported: true,
            reason: None,
        },
        None => ControlCapability {
            supported: false,
            reason: Some("IOS_CONTROL_BLE_HELPER not configured".into()),
        },
    }
}
