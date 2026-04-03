use ios_control_contracts::capture::{
    CaptureCapability, CaptureStreamDescriptor, FrameHealth, SourceKind, VideoSource,
};
use ios_control_frame_transport::FrameSlot;
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};
use std::error::Error;
use std::io::{self, BufRead, Write};

use plugin_capture_window::backend::{allocate_mock_slot, mock_frame, mock_frame_bytes};
use plugin_capture_window::linux_backend::probe_linux_capture;
use plugin_capture_window::windows_backend::probe_windows_capture;

const PROTOCOL_VERSION: u32 = 3;
const SOURCE_ID: &str = "window-1";
const SLOT_BYTES: u32 = (1280 * 720 * 4) as u32;

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

    while let Some(line) = lines.next() {
        let line = line?;
        let request: HostToPlugin = serde_json::from_str(&line)?;
        match request {
            HostToPlugin::Handshake { .. } => {
                let reply = PluginToHost::HandshakeAck {
                    descriptor: PluginDescriptor {
                        plugin_id: "capture.window".into(),
                        protocol_version: PROTOCOL_VERSION,
                        kind: PluginKind::Capture,
                        display_name: "Window Capture".into(),
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
                    message: "handshake required for capture-window plugin".into(),
                };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::ProbeCapture => {
                let capability = CaptureCapability {
                    available: probe_linux_capture() || probe_windows_capture(),
                    reason: if probe_linux_capture() || probe_windows_capture() {
                        None
                    } else {
                        Some("window capture backend unavailable".into())
                    },
                    backend_id: "capture.window".into(),
                    supports_input_bridge: true,
                };
                let reply = PluginToHost::CaptureCapability { capability };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::ListCaptureSources => {
                let sources = if probe_linux_capture() || probe_windows_capture() {
                    vec![VideoSource {
                        source_id: SOURCE_ID.into(),
                        display_name: "Mock Window".into(),
                        kind: SourceKind::Window,
                    }]
                } else {
                    Vec::new()
                };
                let reply = PluginToHost::CaptureSources { sources };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::OpenCaptureStream { source_id } => {
                if source_id != SOURCE_ID {
                    let reply = PluginToHost::Error {
                        message: "unsupported source for capture-window plugin".into(),
                    };
                    write_reply(&mut stdout, &reply)?;
                    continue;
                }

                if !probe_linux_capture() && !probe_windows_capture() {
                    let reply = PluginToHost::Error {
                        message: "window capture backend unavailable".into(),
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
                    source_kind: SourceKind::Window,
                    width: 1280,
                    height: 720,
                    rotation_degrees: 0,
                    slot_bytes: SLOT_BYTES,
                    slot_path: slot.path().display().to_string(),
                };
                stream = Some(StreamState {
                    source_id,
                    frame_index: 0,
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
                let bytes = mock_frame_bytes();
                if let Err(err) = state.slot.write(&bytes) {
                    let reply = PluginToHost::Error {
                        message: format!("failed to write frame slot: {}", err),
                    };
                    write_reply(&mut stdout, &reply)?;
                    continue;
                }

                let mut frame = mock_frame(&state.source_id, state.frame_index);
                frame.health = FrameHealth::Healthy;
                let reply = PluginToHost::CaptureFrame { frame };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::CloseCaptureStream => {
                stream = None;
                write_reply(&mut stdout, &PluginToHost::Ack)?;
            }
            HostToPlugin::GetCaptureFrame { source_id } => {
                if source_id != SOURCE_ID {
                    let reply = PluginToHost::Error {
                        message: "unsupported request for capture-window plugin".into(),
                    };
                    write_reply(&mut stdout, &reply)?;
                } else {
                    legacy_frame_index += 1;
                    let reply = PluginToHost::CaptureFrame {
                        frame: mock_frame(&source_id, legacy_frame_index),
                    };
                    write_reply(&mut stdout, &reply)?;
                }
            }
            _ => {
                let reply = PluginToHost::Error {
                    message: "unsupported request for capture-window plugin".into(),
                };
                write_reply(&mut stdout, &reply)?;
            }
        }
    }

    Ok(())
}
