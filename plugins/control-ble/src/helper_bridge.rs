use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BleHelperProbe {
    pub supported: bool,
    pub supports_prepare: bool,
    pub supports_execute: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BleHelperExecution {
    pub phase: String,
    pub summary: String,
    #[serde(default)]
    pub failure_reason: Option<String>,
}

pub fn run_probe(helper: &Path) -> Result<BleHelperProbe> {
    let output = Command::new(helper).arg("probe").output()?;
    if !output.status.success() {
        return Err(anyhow!("ble helper probe failed"));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub fn run_prepare(helper: &Path) -> Result<()> {
    let output = Command::new(helper).arg("prepare").output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow!("ble helper prepare failed"))
    }
}

pub fn run_execute(helper: &Path, plan_kind: &str) -> Result<BleHelperExecution> {
    let output = Command::new(helper)
        .args(["execute", "--plan-kind", plan_kind])
        .output()?;
    if !output.status.success() {
        return Err(anyhow!("ble helper execute failed"));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}
