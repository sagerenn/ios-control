use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HelperProbe {
    pub available: bool,
    pub display_name: String,
    pub supports_input_bridge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HelperFrameEvent {
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    pub fill_byte: u8,
}

pub fn run_probe(helper: &Path) -> Result<HelperProbe> {
    let output = Command::new(helper).arg("probe").output()?;
    if !output.status.success() {
        return Err(anyhow!("window helper probe failed"));
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
    let event: HelperFrameEvent = serde_json::from_str(&line)?;
    let _ = child.kill();
    let _ = child.wait();
    Ok(event)
}
