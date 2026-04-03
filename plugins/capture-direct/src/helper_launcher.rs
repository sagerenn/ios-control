use crate::helper_bridge::HelperFrameEvent;
use anyhow::{anyhow, Result};
use ios_control_contracts::capture::CaptureCapability;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Stdio};

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

pub fn read_next_frame_event(helper: &Path, source_id: &str) -> Result<HelperFrameEvent> {
    let mut child = Command::new(helper)
        .args(["stream", "--source", source_id])
        .stdout(Stdio::piped())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("missing helper stdout"))?;
    let mut lines = BufReader::new(stdout).lines();
    let line = lines
        .next()
        .ok_or_else(|| anyhow!("missing frame event"))??;
    let event = serde_json::from_str(&line)?;
    let _ = child.kill();
    let _ = child.wait();
    Ok(event)
}
