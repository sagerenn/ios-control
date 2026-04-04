use ios_control_contracts::capture::{CaptureStreamDescriptor, SourceKind};
use ios_control_frame_transport::FrameSlot;
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};
use std::error::Error;
use std::io::{self, BufRead, Write};

use plugin_capture_direct::backend::{allocate_mock_slot, mock_frame, DIRECT_HEIGHT, DIRECT_WIDTH};
use plugin_capture_direct::helper_launcher::{
    capture_capability, find_helper, read_next_frame_event, run_probe,
};

const PROTOCOL_VERSION: u32 = 3;
const SOURCE_ID: &str = "direct-1";
const SLOT_BYTES: u32 = DIRECT_WIDTH * DIRECT_HEIGHT * 4;

struct StreamState {
    source_id: String,
    helper_path: std::path::PathBuf,
    last_frame_index: u64,
    slot: FrameSlot,
}

fn resolve_available_helper() -> Result<std::path::PathBuf, String> {
    let helper_path =
        find_helper().ok_or_else(|| "direct receiver helper not configured".to_string())?;
    match run_probe(&helper_path) {
        Ok(probe) if probe.available => Ok(helper_path),
        Ok(_) => Err("direct receiver helper unavailable".into()),
        Err(err) => Err(format!("incompatible direct helper probe: {}", err)),
    }
}

fn write_reply(stdout: &mut impl Write, reply: &PluginToHost) -> Result<(), Box<dyn Error>> {
    let payload = serde_json::to_string(reply)?;
    stdout.write_all(payload.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    if run_helper_mode()? {
        return Ok(());
    }

    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout());
    let mut lines = stdin.lock().lines();
    let mut handshaken = false;
    let mut stream: Option<StreamState> = None;
    let mut legacy_frame_index: u64 = 0;

    while let Some(line) = lines.next() {
        let line = line?;
        let request: HostToPlugin = serde_json::from_str(&line)?;
        match request {
            HostToPlugin::Handshake { .. } => {
                let reply = PluginToHost::HandshakeAck {
                    descriptor: PluginDescriptor {
                        plugin_id: "capture.direct".into(),
                        protocol_version: PROTOCOL_VERSION,
                        kind: PluginKind::Capture,
                        display_name: "Direct Receiver".into(),
                    },
                };
                handshaken = true;
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::Stop => {
                write_reply(&mut stdout, &PluginToHost::Ack)?;
                break;
            }
            _ if !handshaken => {
                let reply = PluginToHost::Error {
                    message: "handshake required for capture-direct plugin".into(),
                };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::ProbeCapture => {
                let capability = capture_capability(find_helper());
                let reply = PluginToHost::CaptureCapability { capability };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::OpenCaptureStream { source_id } => {
                if source_id != SOURCE_ID {
                    let reply = PluginToHost::Error {
                        message: "unsupported source for capture-direct plugin".into(),
                    };
                    write_reply(&mut stdout, &reply)?;
                    continue;
                }

                let helper_path = match resolve_available_helper() {
                    Ok(path) => path,
                    Err(message) => {
                        let reply = PluginToHost::Error { message };
                        write_reply(&mut stdout, &reply)?;
                        continue;
                    }
                };

                let slot = match allocate_mock_slot() {
                    Ok(slot) => slot,
                    Err(err) => {
                        let reply = PluginToHost::Error {
                            message: format!("failed to allocate frame slot: {}", err),
                        };
                        write_reply(&mut stdout, &reply)?;
                        continue;
                    }
                };

                let descriptor = CaptureStreamDescriptor {
                    source_id: source_id.clone(),
                    source_kind: SourceKind::DirectReceiver,
                    width: DIRECT_WIDTH,
                    height: DIRECT_HEIGHT,
                    rotation_degrees: 0,
                    slot_bytes: SLOT_BYTES,
                    slot_path: slot.path().display().to_string(),
                };
                stream = Some(StreamState {
                    source_id,
                    helper_path,
                    last_frame_index: 0,
                    slot,
                });
                let reply = PluginToHost::CaptureStreamOpened { stream: descriptor };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::ReadCaptureFrame => {
                let state = match stream.as_mut() {
                    Some(state) => state,
                    None => {
                        let reply = PluginToHost::Error {
                            message: "capture stream not open".into(),
                        };
                        write_reply(&mut stdout, &reply)?;
                        continue;
                    }
                };

                let event = match read_next_frame_event(&state.helper_path, &state.source_id) {
                    Ok(event) => event,
                    Err(err) => {
                        let reply = PluginToHost::Error {
                            message: format!("failed to read helper frame event: {}", err),
                        };
                        write_reply(&mut stdout, &reply)?;
                        continue;
                    }
                };
                if event.width != DIRECT_WIDTH || event.height != DIRECT_HEIGHT {
                    let reply = PluginToHost::Error {
                        message: format!(
                            "helper frame geometry mismatch: expected {}x{}, got {}x{}",
                            DIRECT_WIDTH, DIRECT_HEIGHT, event.width, event.height
                        ),
                    };
                    write_reply(&mut stdout, &reply)?;
                    continue;
                }

                let bytes = vec![event.fill_byte; state.slot.byte_len()];
                if let Err(err) = state.slot.write(&bytes) {
                    let reply = PluginToHost::Error {
                        message: format!("failed to write frame slot: {}", err),
                    };
                    write_reply(&mut stdout, &reply)?;
                    continue;
                }

                let frame_index = if event.frame_index > state.last_frame_index {
                    event.frame_index
                } else {
                    state.last_frame_index.saturating_add(1)
                };
                state.last_frame_index = frame_index;
                let mut frame = mock_frame(&state.source_id, frame_index);
                frame.width = event.width;
                frame.height = event.height;
                let reply = PluginToHost::CaptureFrame { frame };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::CloseCaptureStream => {
                stream = None;
                write_reply(&mut stdout, &PluginToHost::Ack)?;
            }
            HostToPlugin::StartDirectCapture => {
                let helper_path = match resolve_available_helper() {
                    Ok(path) => path,
                    Err(message) => {
                        let reply = PluginToHost::Error { message };
                        write_reply(&mut stdout, &reply)?;
                        continue;
                    }
                };

                let event = match read_next_frame_event(&helper_path, SOURCE_ID) {
                    Ok(event) => event,
                    Err(err) => {
                        let reply = PluginToHost::Error {
                            message: format!("failed to read helper frame event: {}", err),
                        };
                        write_reply(&mut stdout, &reply)?;
                        continue;
                    }
                };
                if event.width != DIRECT_WIDTH || event.height != DIRECT_HEIGHT {
                    let reply = PluginToHost::Error {
                        message: format!(
                            "helper frame geometry mismatch: expected {}x{}, got {}x{}",
                            DIRECT_WIDTH, DIRECT_HEIGHT, event.width, event.height
                        ),
                    };
                    write_reply(&mut stdout, &reply)?;
                    continue;
                }

                let frame_index = if event.frame_index > legacy_frame_index {
                    event.frame_index
                } else {
                    legacy_frame_index.saturating_add(1)
                };
                legacy_frame_index = frame_index;
                let mut frame = mock_frame(SOURCE_ID, frame_index);
                frame.width = event.width;
                frame.height = event.height;
                let reply = PluginToHost::CaptureFrame { frame };
                write_reply(&mut stdout, &reply)?;
            }
            _ => {
                let reply = PluginToHost::Error {
                    message: "unsupported request for capture-direct plugin".into(),
                };
                write_reply(&mut stdout, &reply)?;
            }
        }
    }

    Ok(())
}

fn run_helper_mode() -> Result<bool, Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let Some(mode) = args.next() else {
        return Ok(false);
    };

    match mode.as_str() {
        "probe" => {
            let payload = serde_json::json!({
                "available": true,
                "supports_input_bridge": false
            });
            println!("{}", serde_json::to_string(&payload)?);
            Ok(true)
        }
        "stream" => {
            let _ = args.next();
            let _ = args.next();
            let payload = serde_json::json!({
                "frame_index": 1_u64,
                "width": DIRECT_WIDTH,
                "height": DIRECT_HEIGHT,
                "fill_byte": 64_u8
            });
            println!("{}", serde_json::to_string(&payload)?);
            Ok(true)
        }
        _ => Ok(false),
    }
}
