use ios_control_contracts::capture::CaptureCapability;
use std::path::PathBuf;

pub fn find_helper() -> Option<PathBuf> {
    std::env::var_os("IOS_CONTROL_DIRECT_RECEIVER_HELPER")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

pub fn capture_capability(helper: Option<PathBuf>) -> CaptureCapability {
    match helper {
        Some(_) => CaptureCapability {
            available: true,
            reason: None,
            backend_id: "capture.direct.helper".into(),
            supports_input_bridge: false,
        },
        None => CaptureCapability {
            available: false,
            reason: Some("IOS_CONTROL_DIRECT_RECEIVER_HELPER not configured".into()),
            backend_id: "capture.direct.helper".into(),
            supports_input_bridge: false,
        },
    }
}
