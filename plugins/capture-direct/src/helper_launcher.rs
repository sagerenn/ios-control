use crate::helper_bridge::{HelperFrameEvent, HelperProbe};
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
        Some(path) => match run_probe(&path) {
            Ok(probe) => CaptureCapability {
                available: probe.available,
                reason: (!probe.available).then_some("direct receiver helper unavailable".into()),
                backend_id: "capture.direct.helper".into(),
                supports_input_bridge: probe.supports_input_bridge,
            },
            Err(err) => CaptureCapability {
                available: false,
                reason: Some(format!("incompatible helper probe: {}", err)),
                backend_id: "capture.direct.helper".into(),
                supports_input_bridge: false,
            },
        },
        None => CaptureCapability {
            available: false,
            reason: Some("IOS_CONTROL_DIRECT_RECEIVER_HELPER not configured".into()),
            backend_id: "capture.direct.helper".into(),
            supports_input_bridge: false,
        },
    }
}

pub fn run_probe(helper: &Path) -> Result<HelperProbe> {
    let output = Command::new(helper).arg("probe").output()?;
    if !output.status.success() {
        return Err(anyhow!("direct helper probe failed"));
    }
    serde_json::from_slice(&output.stdout).map_err(Into::into)
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
