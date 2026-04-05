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
            resolve_window_capture_helper(),
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

pub fn resolve_window_capture_helper() -> Option<PathBuf> {
    std::env::var_os("IOS_CONTROL_WINDOW_CAPTURE_HELPER")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(resolve_packaged_window_capture_helper)
}

fn resolve_packaged_window_capture_helper() -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok()?;
    let exe_dir = current_exe.parent()?;

    let sibling = exe_dir.join(format!(
        "window-capture-helper{}",
        std::env::consts::EXE_SUFFIX
    ));
    if sibling.is_file() {
        return Some(sibling);
    }

    let bundle_helper = exe_dir.parent().map(|root| {
        root.join("helpers")
            .join(format!("window-capture-helper{}", std::env::consts::EXE_SUFFIX))
    });
    if let Some(path) = bundle_helper.filter(|path| path.is_file()) {
        return Some(path);
    }

    let self_helper = current_exe
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| matches!(*stem, "plugin-capture-window" | "window-capture-helper"))
        .is_some()
        && current_exe.is_file();
    self_helper.then_some(current_exe)
}
