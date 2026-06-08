use crate::backend::ControlCapability;
use crate::helper_bridge::run_probe;
use std::path::PathBuf;

pub fn find_ble_helper() -> Option<PathBuf> {
    std::env::var_os("IOS_CONTROL_BLE_HELPER")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(resolve_packaged_ble_helper)
}

pub fn probe_ble_helper(helper: Option<PathBuf>) -> ControlCapability {
    match helper {
        Some(path) => match run_probe(&path) {
            Ok(probe)
                if probe.supported
                    && probe.supports_prepare
                    && probe.supports_execute
                    && probe.supports_status
                    && probe.supports_stop
                    && probe.supports_forget_bond =>
            {
                ControlCapability {
                    supported: true,
                    reason: None,
                }
            }
            Ok(probe) if !probe.supported => ControlCapability {
                supported: false,
                reason: probe
                    .reason
                    .or_else(|| Some("ble helper reported unsupported".into())),
            },
            Ok(_) => ControlCapability {
                supported: false,
                reason: Some("ble helper missing lifecycle command support".into()),
            },
            Err(err) => ControlCapability {
                supported: false,
                reason: Some(format!("ble helper probe failed: {err}")),
            },
        },
        None => ControlCapability {
            supported: false,
            reason: Some(
                "ble helper not found in override, sibling binary, or bundled helpers directory"
                    .into(),
            ),
        },
    }
}

fn resolve_packaged_ble_helper() -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok()?;
    let exe_dir = current_exe.parent()?;
    let sibling = exe_dir.join(format!("ble-helper{}", std::env::consts::EXE_SUFFIX));
    if sibling.is_file() {
        return Some(sibling);
    }

    let bundle_helper = exe_dir.parent().map(|root| {
        root.join("helpers")
            .join(format!("ble-helper{}", std::env::consts::EXE_SUFFIX))
    });
    bundle_helper.filter(|path| path.is_file())
}
