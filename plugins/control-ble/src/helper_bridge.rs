use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::env;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BleHelperProbe {
    pub supported: bool,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub backend: Option<String>,
    pub supports_prepare: bool,
    pub supports_execute: bool,
    #[serde(default)]
    pub supports_status: bool,
    #[serde(default)]
    pub supports_stop: bool,
    #[serde(default)]
    pub supports_forget_bond: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BleHelperPrepare {
    pub phase: String,
    pub checklist: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub paired_device_id: Option<String>,
    #[serde(default)]
    pub paired_device_name: Option<String>,
    #[serde(default)]
    pub bonded: bool,
    #[serde(default)]
    pub execute_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BleHelperStatus {
    pub phase: String,
    pub checklist: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub paired_device_id: Option<String>,
    #[serde(default)]
    pub paired_device_name: Option<String>,
    #[serde(default)]
    pub bonded: bool,
    #[serde(default)]
    pub execute_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BleHelperExecution {
    pub phase: String,
    pub summary: String,
    #[serde(default)]
    pub observed_change: bool,
    #[serde(default)]
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BleHelperAck {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub message: Option<String>,
}

const DEFAULT_TIMEOUT_MS: u64 = 2_000;
const POLL_INTERVAL_MS: u64 = 10;

fn helper_command(helper: &Path) -> Command {
    if helper.extension().and_then(|ext| ext.to_str()) == Some("sh") {
        let mut command = Command::new("sh");
        command.arg(helper);
        return command;
    }

    Command::new(helper)
}

fn helper_timeout() -> Duration {
    let from_env = env::var("IOS_CONTROL_BLE_HELPER_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    Duration::from_millis(from_env)
}

fn wait_for_completion(child: &mut Child, timeout: Duration, context: &str) -> Result<ExitStatus> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(anyhow!(
                "{context} timed out after {}ms",
                timeout.as_millis()
            ));
        }
        thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }
}

fn run_for_output(mut command: Command, context: &str) -> Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("{context} missing stdout pipe"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("{context} missing stderr pipe"))?;
    let stdout_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut reader = stdout;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(bytes)
    });
    let stderr_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut reader = stderr;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(bytes)
    });

    let timeout = helper_timeout();
    let status = wait_for_completion(&mut child, timeout, context)?;
    let stdout = stdout_handle
        .join()
        .map_err(|_| anyhow!("{context} stdout drainer panicked"))??;
    let stderr = stderr_handle
        .join()
        .map_err(|_| anyhow!("{context} stderr drainer panicked"))??;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

pub fn run_probe(helper: &Path) -> Result<BleHelperProbe> {
    let mut command = helper_command(helper);
    command.arg("probe");
    let output = run_for_output(command, "ble helper probe")?;
    if !output.status.success() {
        return Err(anyhow!("ble helper probe failed"));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub fn run_prepare(helper: &Path) -> Result<BleHelperPrepare> {
    let mut command = helper_command(helper);
    command.arg("prepare");
    let output = run_for_output(command, "ble helper prepare")?;
    if output.status.success() {
        Ok(serde_json::from_slice(&output.stdout)?)
    } else {
        Err(anyhow!("ble helper prepare failed"))
    }
}

pub fn run_execute(helper: &Path, plan_kind: &str) -> Result<BleHelperExecution> {
    let mut command = helper_command(helper);
    command.args(["execute", "--plan-kind", plan_kind]);
    let output = run_for_output(command, "ble helper execute")?;
    if !output.status.success() {
        return Err(anyhow!("ble helper execute failed"));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub fn run_status(helper: &Path) -> Result<BleHelperStatus> {
    let mut command = helper_command(helper);
    command.arg("status");
    let output = run_for_output(command, "ble helper status")?;
    if !output.status.success() {
        return Err(anyhow!("ble helper status failed"));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub fn run_stop(helper: &Path) -> Result<BleHelperAck> {
    let mut command = helper_command(helper);
    command.arg("stop");
    let output = run_for_output(command, "ble helper stop")?;
    if !output.status.success() {
        return Err(anyhow!("ble helper stop failed"));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub fn run_forget_bond(helper: &Path, device_id: &str) -> Result<BleHelperAck> {
    let mut command = helper_command(helper);
    command.args(["forget-bond", "--device", device_id]);
    let output = run_for_output(command, "ble helper forget-bond")?;
    if !output.status.success() {
        return Err(anyhow!("ble helper forget-bond failed"));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}
