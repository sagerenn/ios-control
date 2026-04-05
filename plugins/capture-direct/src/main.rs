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
const BASE64_CHARS: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

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

fn encode_base64_bytes(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut i = 0usize;
    while i + 3 <= input.len() {
        let b0 = input[i];
        let b1 = input[i + 1];
        let b2 = input[i + 2];
        out.push(BASE64_CHARS[(b0 >> 2) as usize] as char);
        out.push(BASE64_CHARS[((b0 & 0x03) << 4 | (b1 >> 4)) as usize] as char);
        out.push(BASE64_CHARS[((b1 & 0x0f) << 2 | (b2 >> 6)) as usize] as char);
        out.push(BASE64_CHARS[(b2 & 0x3f) as usize] as char);
        i += 3;
    }

    match input.len() - i {
        1 => {
            let b0 = input[i];
            out.push(BASE64_CHARS[(b0 >> 2) as usize] as char);
            out.push(BASE64_CHARS[((b0 & 0x03) << 4) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let b0 = input[i];
            let b1 = input[i + 1];
            out.push(BASE64_CHARS[(b0 >> 2) as usize] as char);
            out.push(BASE64_CHARS[((b0 & 0x03) << 4 | (b1 >> 4)) as usize] as char);
            out.push(BASE64_CHARS[((b1 & 0x0f) << 2) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
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

                let bytes = match event.decode_rgba() {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        let reply = PluginToHost::Error {
                            message: format!("failed to decode helper frame payload: {}", err),
                        };
                        write_reply(&mut stdout, &reply)?;
                        continue;
                    }
                };
                if bytes.len() != state.slot.byte_len() {
                    let reply = PluginToHost::Error {
                        message: format!(
                            "helper frame payload size mismatch: expected {}, got {}",
                            state.slot.byte_len(),
                            bytes.len()
                        ),
                    };
                    write_reply(&mut stdout, &reply)?;
                    continue;
                }
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
                frame.rotation_degrees = event.rotation_degrees;
                frame.health = event.health;
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
                let bytes = match event.decode_rgba() {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        let reply = PluginToHost::Error {
                            message: format!("failed to decode helper frame payload: {}", err),
                        };
                        write_reply(&mut stdout, &reply)?;
                        continue;
                    }
                };
                if bytes.len() != SLOT_BYTES as usize {
                    let reply = PluginToHost::Error {
                        message: format!(
                            "helper frame payload size mismatch: expected {}, got {}",
                            SLOT_BYTES,
                            bytes.len()
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
                frame.rotation_degrees = event.rotation_degrees;
                frame.health = event.health;
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
            let rgba = encode_base64_bytes(&vec![64_u8; SLOT_BYTES as usize]);
            let payload = serde_json::json!({
                "frame_index": 1_u64,
                "width": DIRECT_WIDTH,
                "height": DIRECT_HEIGHT,
                "rotation_degrees": 0_u16,
                "health": "Healthy",
                "rgba_base64": rgba
            });
            println!("{}", serde_json::to_string(&payload)?);
            Ok(true)
        }
        _ => Ok(false),
    }
}
