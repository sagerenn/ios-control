use ios_control_contracts::capture::{
    CaptureCapability, CaptureStreamDescriptor, FrameHealth, SourceKind,
};
use ios_control_frame_transport::FrameSlot;
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};
use std::error::Error;
use std::io::{self, BufRead, Write};

use plugin_capture_window::backend::{allocate_mock_slot, mock_frame};
use plugin_capture_window::helper_bridge;
use plugin_capture_window::helper_config::WindowHelperConfig;
use plugin_capture_window::helper_config::WINDOW_HELPER_SOURCE_ID;

const PROTOCOL_VERSION: u32 = 3;
const SLOT_BYTES: u32 = (1280 * 720 * 4) as u32;
const STREAM_WIDTH: u32 = 1280;
const STREAM_HEIGHT: u32 = 720;

struct StreamState {
    source_id: String,
    helper_path: std::path::PathBuf,
    last_frame_index: u64,
    slot: FrameSlot,
}

enum WindowHelperStatus {
    Unconfigured,
    ProbeFailed,
    Unavailable {
        supports_input_bridge: bool,
    },
    Available {
        config: WindowHelperConfig,
        probe: helper_bridge::HelperProbe,
    },
}

fn window_helper_status() -> WindowHelperStatus {
    let Some(config) = WindowHelperConfig::from_env() else {
        return WindowHelperStatus::Unconfigured;
    };
    match helper_bridge::run_probe(&config.helper_path) {
        Ok(probe) => {
            if probe.available {
                WindowHelperStatus::Available { config, probe }
            } else {
                WindowHelperStatus::Unavailable {
                    supports_input_bridge: probe.supports_input_bridge,
                }
            }
        }
        Err(_) => WindowHelperStatus::ProbeFailed,
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
                let capability = match window_helper_status() {
                    WindowHelperStatus::Available { probe, .. } => CaptureCapability {
                        available: true,
                        reason: None,
                        backend_id: "capture.window.helper".into(),
                        supports_input_bridge: probe.supports_input_bridge,
                    },
                    WindowHelperStatus::Unavailable {
                        supports_input_bridge,
                    } => CaptureCapability {
                        available: false,
                        reason: Some("window capture helper unavailable".into()),
                        backend_id: "capture.window.helper".into(),
                        supports_input_bridge,
                    },
                    WindowHelperStatus::ProbeFailed => CaptureCapability {
                        available: false,
                        reason: Some("window helper probe failed".into()),
                        backend_id: "capture.window.helper".into(),
                        supports_input_bridge: false,
                    },
                    WindowHelperStatus::Unconfigured => CaptureCapability {
                        available: false,
                        reason: Some("IOS_CONTROL_WINDOW_CAPTURE_HELPER not configured".into()),
                        backend_id: "capture.window.helper".into(),
                        supports_input_bridge: false,
                    },
                };
                let reply = PluginToHost::CaptureCapability { capability };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::ListCaptureSources => {
                let sources = match window_helper_status() {
                    WindowHelperStatus::Available { config, probe } => {
                        config.list_sources_with_name(&probe.display_name)
                    }
                    _ => Vec::new(),
                };
                let reply = PluginToHost::CaptureSources { sources };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::OpenCaptureStream { source_id } => {
                if source_id != WINDOW_HELPER_SOURCE_ID {
                    let reply = PluginToHost::Error {
                        message: "unsupported source for capture-window plugin".into(),
                    };
                    write_reply(&mut stdout, &reply)?;
                    continue;
                }

                let config = match window_helper_status() {
                    WindowHelperStatus::Available { config, .. } => config,
                    WindowHelperStatus::Unconfigured => {
                        let reply = PluginToHost::Error {
                            message: "window capture helper not configured".into(),
                        };
                        write_reply(&mut stdout, &reply)?;
                        continue;
                    }
                    WindowHelperStatus::ProbeFailed => {
                        let reply = PluginToHost::Error {
                            message: "window helper probe failed".into(),
                        };
                        write_reply(&mut stdout, &reply)?;
                        continue;
                    }
                    WindowHelperStatus::Unavailable { .. } => {
                        let reply = PluginToHost::Error {
                            message: "window capture helper unavailable".into(),
                        };
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
                    source_kind: SourceKind::Window,
                    width: STREAM_WIDTH,
                    height: STREAM_HEIGHT,
                    rotation_degrees: 0,
                    slot_bytes: SLOT_BYTES,
                    slot_path: slot.path().display().to_string(),
                };
                stream = Some(StreamState {
                    source_id,
                    helper_path: config.helper_path,
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

                let event = match helper_bridge::read_next_frame_event(
                    &state.helper_path,
                    &state.source_id,
                ) {
                    Ok(event) => event,
                    Err(err) => {
                        let reply = PluginToHost::Error {
                            message: format!("failed to read helper frame event: {}", err),
                        };
                        write_reply(&mut stdout, &reply)?;
                        continue;
                    }
                };
                if event.width != STREAM_WIDTH || event.height != STREAM_HEIGHT {
                    let reply = PluginToHost::Error {
                        message: format!(
                            "helper frame geometry mismatch: expected {}x{}, got {}x{}",
                            STREAM_WIDTH, STREAM_HEIGHT, event.width, event.height
                        ),
                    };
                    write_reply(&mut stdout, &reply)?;
                    continue;
                }
                let bytes = if event.rgba_base64.is_empty() {
                    vec![event.fill_byte; state.slot.byte_len()]
                } else {
                    event.decode_rgba().map_err(Box::<dyn Error>::from)?
                };
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
                frame.health = FrameHealth::Healthy;
                let reply = PluginToHost::CaptureFrame { frame };
                write_reply(&mut stdout, &reply)?;
            }
            HostToPlugin::CloseCaptureStream => {
                stream = None;
                write_reply(&mut stdout, &PluginToHost::Ack)?;
            }
            HostToPlugin::GetCaptureFrame { source_id } => {
                if source_id != WINDOW_HELPER_SOURCE_ID {
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

fn run_helper_mode() -> Result<bool, Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let Some(mode) = args.next() else {
        return Ok(false);
    };

    match mode.as_str() {
        "probe" => {
            let display_name = std::env::var("IOS_CONTROL_WINDOW_CAPTURE_NAME")
                .unwrap_or_else(|_| "Operator Mirror".into());
            let payload = serde_json::json!({
                "available": true,
                "display_name": display_name,
                "supports_input_bridge": true
            });
            println!("{}", serde_json::to_string(&payload)?);
            Ok(true)
        }
        "stream" => {
            let _ = args.next();
            let _ = args.next();
            let payload = serde_json::json!({
                "frame_index": 1_u64,
                "width": 1280_u32,
                "height": 720_u32,
                "fill_byte": 128_u8
            });
            println!("{}", serde_json::to_string(&payload)?);
            Ok(true)
        }
        _ => Ok(false),
    }
}
