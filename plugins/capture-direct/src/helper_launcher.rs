use crate::helper_bridge::{HelperFrameEvent, HelperProbe};
use crate::runtime_bundle::DirectRuntimeBundle;
use anyhow::{anyhow, Result};
use ios_control_contracts::capture::CaptureCapability;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const HELPER_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

fn helper_command(helper: &Path) -> Command {
    if helper.extension().and_then(|ext| ext.to_str()) == Some("sh") {
        let mut command = Command::new("/bin/sh");
        command.arg(helper);
        return command;
    }

    Command::new(helper)
}

pub fn find_helper() -> Option<PathBuf> {
    std::env::var_os("IOS_CONTROL_DIRECT_RECEIVER_HELPER")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

pub fn capture_capability(helper: Option<PathBuf>) -> CaptureCapability {
    if DirectRuntimeBundle::configured_root().is_some() {
        return match DirectRuntimeBundle::resolve().and_then(|bundle| bundle.probe()) {
            Ok(()) => CaptureCapability {
                available: true,
                reason: None,
                backend_id: "capture.direct.uxplay".into(),
                supports_input_bridge: false,
            },
            Err(err) => CaptureCapability {
                available: false,
                reason: Some(err.to_string()),
                backend_id: "capture.direct.uxplay".into(),
                supports_input_bridge: false,
            },
        };
    }

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

    let status = wait_for_exit(&mut child, "direct helper probe")?;
    if !status.success() {
        drop(reader);
        return Err(anyhow!("direct helper probe failed"));
    }
    let bytes = match rx.recv_timeout(HELPER_TIMEOUT) {
        Ok(result) => result?,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            drop(reader);
            return Err(anyhow!(
                "direct helper probe stdout read timed out after {:?}",
                HELPER_TIMEOUT
            ));
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            drop(reader);
            return Err(anyhow!("direct helper probe stdout read failed"));
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
            let event = serde_json::from_str(&line)?;
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
                "direct helper frame event read timed out after {:?}",
                HELPER_TIMEOUT
            ));
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = child.kill();
            let _ = child.wait();
            drop(reader);
            return Err(anyhow!("direct helper frame event read failed"));
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
