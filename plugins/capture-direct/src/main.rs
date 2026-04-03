use ios_control_contracts::capture::{CaptureCapability, CaptureStreamDescriptor, SourceKind};
use ios_control_frame_transport::FrameSlot;
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};
use std::error::Error;
use std::io::{self, BufRead, Write};

use plugin_capture_direct::backend::{
    allocate_mock_slot, mock_frame, mock_frame_bytes, DIRECT_HEIGHT, DIRECT_WIDTH,
};
use plugin_capture_direct::helper_launcher::find_helper;

const PROTOCOL_VERSION: u32 = 3;
const SOURCE_ID: &str = "direct-1";
const SLOT_BYTES: u32 = DIRECT_WIDTH * DIRECT_HEIGHT * 4;

struct StreamState {
    source_id: String,
    frame_index: u64,
    slot: FrameSlot,
}

fn write_reply(stdout: &mut impl Write, reply: &PluginToHost) -> Result<(), Box<dyn Error>> {
    let payload = serde_json::to_string(reply)?;
    stdout.write_all(payload.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout());
    let mut lines = stdin.lock().lines();
    let mut handshaken = false;
    let mut stream: Option<StreamState> = None;
    let mut legacy_frame_index: u64 = 0;
    let mut stream_frame_index: u64 = 0;

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
                let capability = CaptureCapability {
                    available: find_helper().is_some(),
                    reason: if find_helper().is_some() {
                        None
                    } else {
                        Some("direct receiver helper not configured".into())
                    },
                    backend_id: "capture.direct".into(),
                    supports_input_bridge: false,
                };
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

                if find_helper().is_none() {
                    let reply = PluginToHost::Error {
                        message: "direct receiver helper not configured".into(),
                    };
                    write_reply(&mut stdout, &reply)?;
                    continue;
                }

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
                    frame_index: stream_frame_index,
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

                state.frame_index += 1;
                stream_frame_index = state.frame_index;
                let bytes = mock_frame_bytes();
                if let Err(err) = state.slot.write(&bytes) {
                    let reply = PluginToHost::Error {
                        message: format!("failed to write frame slot: {}", err),
                    };
                    write_reply(&mut stdout, &reply)?;
                    continue;
                }

                let frame = mock_frame(&state.source_id, state.frame_index);
                let reply = PluginToHost::CaptureFrame { frame };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::CloseCaptureStream => {
                stream = None;
                write_reply(&mut stdout, &PluginToHost::Ack)?;
            }
            HostToPlugin::StartDirectCapture => {
                legacy_frame_index += 1;
                let frame_index = legacy_frame_index;
                let reply = PluginToHost::CaptureFrame {
                    frame: mock_frame(SOURCE_ID, frame_index),
                };
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
