use anyhow::{anyhow, Result};
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
    pub fill_byte: u8,
    #[serde(default)]
    pub rgba_base64: String,
}

impl HelperFrameEvent {
    pub fn decode_rgba(&self) -> Result<Vec<u8>> {
        decode_base64(&self.rgba_base64)
    }
}

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

fn decode_base64(input: &str) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut chunk = [0_u8; 4];
    let mut len = 0_usize;

    for byte in input.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        chunk[len] = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => 64,
            _ => return Err(anyhow!("invalid base64 byte")),
        };
        len += 1;

        if len == 4 {
            if chunk[0] == 64 || chunk[1] == 64 {
                return Err(anyhow!("invalid base64 padding"));
            }

            output.push((chunk[0] << 2) | (chunk[1] >> 4));
            if chunk[2] != 64 {
                output.push((chunk[1] << 4) | (chunk[2] >> 2));
                if chunk[3] != 64 {
                    output.push((chunk[2] << 6) | chunk[3]);
                }
            } else if chunk[3] != 64 {
                return Err(anyhow!("invalid base64 padding"));
            }

            len = 0;
        }
    }

    if len != 0 {
        return Err(anyhow!("invalid base64 length"));
    }

    Ok(output)
}
