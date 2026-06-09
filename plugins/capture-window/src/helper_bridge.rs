use anyhow::{anyhow, Result};
use ios_control_contracts::capture::FrameHealth;
use ios_control_frame_transport::decode_base64_bytes;
use serde::Deserialize;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

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
    #[serde(default)]
    pub rotation_degrees: u16,
    #[serde(default = "default_frame_health")]
    pub health: FrameHealth,
    pub rgba_base64: String,
}

fn default_frame_health() -> FrameHealth {
    FrameHealth::Healthy
}

impl HelperFrameEvent {
    pub fn decode_rgba(&self) -> Result<Vec<u8>> {
        decode_base64_bytes(&self.rgba_base64)
    }
}

const HELPER_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

fn helper_command(helper: &Path) -> Command {
    if helper.extension().and_then(|ext| ext.to_str()) == Some("sh") {
        let mut command = Command::new("/bin/sh");
        hide_child_console(&mut command);
        command.arg(helper);
        return command;
    }

    let mut command = Command::new(helper);
    hide_child_console(&mut command);
    command
}

fn hide_child_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
}

pub fn run_probe(helper: &Path) -> Result<HelperProbe> {
    let mut child = helper_command(helper)
        .arg("probe")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("missing helper stdout"))?;
    let (tx, rx) = mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut stdout = stdout;
        let result = (|| {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes)?;
            Result::<Vec<u8>>::Ok(bytes)
        })();
        let _ = tx.send(result);
    });

    let status = wait_for_exit(&mut child, "window helper probe")?;
    if !status.success() {
        drop(reader);
        return Err(anyhow!("window helper probe failed"));
    }
    let bytes = match rx.recv_timeout(HELPER_TIMEOUT) {
        Ok(result) => result?,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            drop(reader);
            return Err(anyhow!(
                "window helper probe stdout read timed out after {:?}",
                HELPER_TIMEOUT
            ));
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            drop(reader);
            return Err(anyhow!("window helper probe stdout read failed"));
        }
    };
    let _ = reader.join();
    serde_json::from_slice(&bytes).map_err(Into::into)
}

pub fn read_next_frame_event(helper: &Path, source_id: &str) -> Result<HelperFrameEvent> {
    let mut child = helper_command(helper)
        .args(["stream", "--source", source_id])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("missing helper stdout"))?;

    let (tx, rx) = mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut lines = BufReader::new(stdout).lines();
        let result = (|| {
            let line = lines
                .next()
                .ok_or_else(|| anyhow!("missing frame event"))??;
            let event: HelperFrameEvent = serde_json::from_str(&line)?;
            Result::<HelperFrameEvent>::Ok(event)
        })();
        let _ = tx.send(result);
    });

    let event = match rx.recv_timeout(HELPER_TIMEOUT) {
        Ok(result) => result?,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let _ = child.kill();
            let _ = child.wait();
            drop(reader);
            return Err(anyhow!(
                "window helper frame event read timed out after {:?}",
                HELPER_TIMEOUT
            ));
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = child.kill();
            let _ = child.wait();
            drop(reader);
            return Err(anyhow!("window helper frame event read failed"));
        }
    };

    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
    Ok(event)
}

fn wait_for_exit(child: &mut Child, context: &str) -> Result<ExitStatus> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if start.elapsed() >= HELPER_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(anyhow!("{} timed out after {:?}", context, HELPER_TIMEOUT));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}
