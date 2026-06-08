use anyhow::{anyhow, Result};
use ios_control_contracts::control::{
    ControlInputEvent, ExecutionPhase, ExecutionSummary, KeyModifiers, KeyboardInputReport,
    MouseInputReport,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

#[derive(Debug, Clone, Serialize)]
struct HidCommand {
    id: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mouse: Option<MouseCommand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keyboard: Option<KeyboardCommand>,
}

#[derive(Debug, Clone, Serialize)]
struct MouseCommand {
    reports: Vec<MouseReport>,
    delay_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
struct MouseReport {
    buttons: u8,
    dx: i8,
    dy: i8,
    wheel: i8,
    repeat: u16,
}

#[derive(Debug, Clone, Serialize)]
struct KeyboardCommand {
    reports: Vec<KeyboardReport>,
    delay_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
struct KeyboardReport {
    modifiers: u8,
    keys: Vec<u8>,
    repeat: u16,
}

#[derive(Debug, Clone, Deserialize)]
struct HidAck {
    ok: bool,
    message: String,
}

#[derive(Debug, Clone)]
struct HidPaths {
    command_dir: PathBuf,
    ack_dir: PathBuf,
}

const DEFAULT_TIMEOUT_MS: u64 = 2_000;
const POLL_INTERVAL_MS: u64 = 10;
const LIVE_INPUT_TIMEOUT_MS: u64 = 750;

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

pub fn run_control_input(_helper: &Path, event: ControlInputEvent) -> Result<ExecutionSummary> {
    let command = match event {
        ControlInputEvent::Mouse(mouse) => HidCommand {
            id: String::new(),
            kind: "mouse".into(),
            mouse: Some(mouse_command(mouse)),
            keyboard: None,
        },
        ControlInputEvent::Keyboard(keyboard) => HidCommand {
            id: String::new(),
            kind: "keyboard".into(),
            mouse: None,
            keyboard: Some(keyboard_command(keyboard)),
        },
    };

    let ack = enqueue_hid_command(command, Duration::from_millis(LIVE_INPUT_TIMEOUT_MS))?;
    let phase = if ack.ok {
        ExecutionPhase::Succeeded
    } else {
        ExecutionPhase::Failed
    };
    Ok(ExecutionSummary {
        summary: ack.message.clone(),
        phase,
        observed_change: Some(ack.ok),
        failure_reason: (!ack.ok).then_some(ack.message),
    })
}

fn mouse_command(mouse: MouseInputReport) -> MouseCommand {
    MouseCommand {
        reports: split_mouse_reports(mouse),
        delay_ms: 0,
    }
}

fn split_mouse_reports(mouse: MouseInputReport) -> Vec<MouseReport> {
    let mut reports = Vec::new();
    let mut dx = mouse.dx;
    let mut dy = mouse.dy;
    let mut wheel = mouse.wheel;

    loop {
        let step_dx = clamp_hid_delta(dx);
        let step_dy = clamp_hid_delta(dy);
        let step_wheel = wheel;
        wheel = 0;
        reports.push(MouseReport {
            buttons: mouse.buttons & 0x07,
            dx: step_dx,
            dy: step_dy,
            wheel: step_wheel,
            repeat: 1,
        });

        dx -= i16::from(step_dx);
        dy -= i16::from(step_dy);
        if dx == 0 && dy == 0 && wheel == 0 {
            break;
        }
    }

    reports
}

fn clamp_hid_delta(value: i16) -> i8 {
    value.clamp(-127, 127) as i8
}

fn keyboard_command(keyboard: KeyboardInputReport) -> KeyboardCommand {
    let keys = if keyboard.pressed && keyboard.usage_id != 0 {
        vec![keyboard.usage_id]
    } else {
        Vec::new()
    };
    KeyboardCommand {
        reports: vec![KeyboardReport {
            modifiers: modifier_mask(keyboard.modifiers),
            keys,
            repeat: 1,
        }],
        delay_ms: 0,
    }
}

fn modifier_mask(modifiers: KeyModifiers) -> u8 {
    let mut mask = 0u8;
    if modifiers.ctrl {
        mask |= 0x01;
    }
    if modifiers.shift {
        mask |= 0x02;
    }
    if modifiers.alt {
        mask |= 0x04;
    }
    if modifiers.meta {
        mask |= 0x08;
    }
    mask
}

fn enqueue_hid_command(mut command: HidCommand, timeout: Duration) -> Result<HidAck> {
    let paths = HidPaths::new(state_path()?);
    fs::create_dir_all(&paths.command_dir)?;
    fs::create_dir_all(&paths.ack_dir)?;

    let id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    command.id = id.clone();

    let command_path = paths.command_dir.join(format!("{id}.json"));
    let temp_path = paths.command_dir.join(format!("{id}.tmp"));
    fs::write(&temp_path, serde_json::to_string(&command)?)?;
    fs::rename(temp_path, command_path)?;

    let ack_path = paths.ack_dir.join(format!("{id}.json"));
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(text) = fs::read_to_string(&ack_path) {
            let _ = fs::remove_file(&ack_path);
            return Ok(serde_json::from_str(&text)?);
        }
        thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }

    Err(anyhow!("timed out waiting for BLE HID live input response"))
}

impl HidPaths {
    fn new(state_file: PathBuf) -> Self {
        let root = state_file
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(std::env::temp_dir);
        Self {
            command_dir: root.join("ble-helper-hid-commands"),
            ack_dir: root.join("ble-helper-hid-acks"),
        }
    }
}

fn state_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("IOS_CONTROL_BLE_HELPER_STATE_DIR") {
        return Ok(PathBuf::from(path).join("ble-helper-state.json"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Ok(PathBuf::from(appdata)
            .join("ios-control")
            .join("ble-helper-state.json"));
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg)
            .join("ios-control")
            .join("ble-helper-state.json"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(Path::new(&home)
            .join(".config")
            .join("ios-control")
            .join("ble-helper-state.json"));
    }
    Ok(std::env::temp_dir().join("ble-helper-state.json"))
}

#[cfg(test)]
mod live_input_tests {
    use super::*;

    #[test]
    fn mouse_input_splits_large_hid_deltas() {
        let reports = split_mouse_reports(MouseInputReport {
            buttons: 1,
            dx: 300,
            dy: -300,
            wheel: 1,
        });

        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0].dx, 127);
        assert_eq!(reports[0].dy, -127);
        assert_eq!(reports[0].wheel, 1);
        assert_eq!(reports[2].dx, 46);
        assert_eq!(reports[2].dy, -46);
        assert_eq!(reports[2].buttons, 1);
    }

    #[test]
    fn keyboard_input_maps_contract_modifiers() {
        let command = keyboard_command(KeyboardInputReport {
            usage_id: 0x04,
            modifiers: KeyModifiers {
                shift: true,
                meta: true,
                ..Default::default()
            },
            pressed: true,
        });

        assert_eq!(command.reports[0].modifiers, 0x0a);
        assert_eq!(command.reports[0].keys, vec![0x04]);
    }
}
