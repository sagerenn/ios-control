use ios_control_contracts::capture::{CaptureCapability, SourceKind, VideoSource};
use std::path::PathBuf;

pub const WINDOW_HELPER_SOURCE_ID: &str = "window-helper-1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowHelperConfig {
    pub helper_path: PathBuf,
    pub display_name: String,
}

impl WindowHelperConfig {
    pub fn from_parts(helper_path: Option<PathBuf>, display_name: Option<String>) -> Option<Self> {
        helper_path
            .filter(|path| path.is_file())
            .map(|helper_path| Self {
                helper_path,
                display_name: display_name.unwrap_or_else(|| "Operator Mirror".into()),
            })
    }

    pub fn from_env() -> Option<Self> {
        Self::from_parts(
            std::env::var_os("IOS_CONTROL_WINDOW_CAPTURE_HELPER").map(PathBuf::from),
            std::env::var("IOS_CONTROL_WINDOW_CAPTURE_NAME").ok(),
        )
    }

    pub fn capture_capability(&self) -> CaptureCapability {
        CaptureCapability {
            available: true,
            reason: None,
            backend_id: "capture.window.helper".into(),
            supports_input_bridge: true,
        }
    }

    pub fn list_sources(&self) -> Vec<VideoSource> {
        self.list_sources_with_name(&self.display_name)
    }

    pub fn list_sources_with_name(&self, display_name: &str) -> Vec<VideoSource> {
        let display_name = if display_name.is_empty() {
            self.display_name.clone()
        } else {
            display_name.to_string()
        };
        vec![VideoSource {
            source_id: WINDOW_HELPER_SOURCE_ID.into(),
            display_name,
            kind: SourceKind::Window,
        }]
    }
}
