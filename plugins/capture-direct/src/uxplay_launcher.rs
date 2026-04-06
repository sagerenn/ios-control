use anyhow::{anyhow, Context, Result};
use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

use crate::direct_status::DirectCaptureStatus;
use crate::rtp_audio;
use crate::rtp_video::{self, DecodedFrame};
use crate::runtime_bundle::{DirectRuntimeBundle, BLE_PATH_ENV, RUNTIME_ROOT_ENV};

const FRAME_WAIT_TIMEOUT: Duration = Duration::from_secs(2);
const FRAME_WAIT_POLL: Duration = Duration::from_millis(50);

pub struct DirectRuntimeSession {
    _session_dir: TempDir,
    frame_dir: PathBuf,
    ble_path: PathBuf,
    video_receiver: Child,
    audio_receiver: Child,
    beacon: Child,
    uxplay: Child,
    last_frame_index: u64,
}

impl DirectRuntimeSession {
    pub fn start(bundle: &DirectRuntimeBundle, status: &mut DirectCaptureStatus) -> Result<Self> {
        let session_dir = tempfile::tempdir().context("failed to create direct session dir")?;
        let frame_dir = session_dir.path().join("frames");
        std::fs::create_dir_all(&frame_dir)?;
        let ble_path = session_dir.path().join("uxplay.ble");
        let video_port = reserve_udp_port()?;
        let audio_port = reserve_udp_port()?;
        let frame_pattern = frame_dir.join("frame-%09d.png");

        let video_receiver = spawn_child(
            &bundle.gst_launch_path,
            &rtp_video::receiver_args(video_port, &frame_pattern),
            bundle,
            Some(("IOS_CONTROL_DIRECT_FRAME_PATTERN", frame_pattern.as_os_str())),
        )
        .context("failed to launch video receiver")?;
        let audio_receiver = spawn_child(
            &bundle.gst_launch_path,
            &rtp_audio::receiver_args(audio_port),
            bundle,
            None,
        )
        .context("failed to launch audio receiver")?;

        let beacon = spawn_beacon(bundle, &ble_path).context("failed to launch beacon helper")?;
        let uxplay = spawn_uxplay(bundle, &ble_path, video_port, audio_port)
            .context("failed to launch uxplay")?;

        status.waiting_for_runtime_frame();
        status.audio_route = ios_control_contracts::capture::AudioRoute::LocalPlayback;
        status.audio_active = true;
        status.audio_phase = ios_control_contracts::capture::AudioStreamPhase::Waiting;

        Ok(Self {
            _session_dir: session_dir,
            frame_dir,
            ble_path,
            video_receiver,
            audio_receiver,
            beacon,
            uxplay,
            last_frame_index: 0,
        })
    }

    pub fn next_frame(&mut self) -> Result<Option<DecodedFrame>> {
        let start = Instant::now();
        while start.elapsed() < FRAME_WAIT_TIMEOUT {
            if let Some(frame) = rtp_video::next_frame(&self.frame_dir, &mut self.last_frame_index)? {
                return Ok(Some(frame));
            }
            if self.uxplay.try_wait()?.is_some() {
                return Err(anyhow!("uxplay exited before a frame was produced"));
            }
            if let Some(exit) = self.video_receiver.try_wait()? {
                return Err(anyhow!(
                    "video receiver exited with status {exit} before a frame was produced"
                ));
            }
            std::thread::sleep(FRAME_WAIT_POLL);
        }
        Ok(None)
    }

    pub fn refresh_status(&mut self, status: &mut DirectCaptureStatus) -> Result<()> {
        if let Some(exit) = self.uxplay.try_wait()? {
            status.detail = Some(format!("UxPlay exited with status {exit}"));
            status.video_phase = ios_control_contracts::capture::CaptureStreamPhase::Error;
        }
        if let Some(exit) = self.video_receiver.try_wait()? {
            status.detail = Some(format!("Video receiver exited with status {exit}"));
            status.video_phase = ios_control_contracts::capture::CaptureStreamPhase::Error;
        }
        if let Some(exit) = self.audio_receiver.try_wait()? {
            status.detail = Some(format!("Audio receiver exited with status {exit}"));
            status.audio_phase = ios_control_contracts::capture::AudioStreamPhase::Degraded;
            status.audio_active = false;
        }
        if let Some(exit) = self.beacon.try_wait()? {
            status.detail = Some(format!("Beacon helper exited with status {exit}"));
        }
        if self.ble_path.is_file() && status.detail.is_none() {
            status.detail = Some("Waiting for forwarded frame data".into());
        }
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<()> {
        let _ = kill_child(&mut self.uxplay);
        let _ = kill_child(&mut self.beacon);
        let _ = kill_child(&mut self.video_receiver);
        let _ = kill_child(&mut self.audio_receiver);
        Ok(())
    }
}

fn reserve_udp_port() -> Result<u16> {
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    Ok(socket.local_addr()?.port())
}

fn spawn_beacon(bundle: &DirectRuntimeBundle, ble_path: &std::path::Path) -> Result<Child> {
    let mut command = Command::new(&bundle.beacon_helper_path);
    command
        .arg("serve")
        .env(RUNTIME_ROOT_ENV, &bundle.root)
        .env(BLE_PATH_ENV, ble_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_runtime_env(&mut command, bundle);
    command.spawn().context("failed to spawn direct beacon helper")
}

fn spawn_uxplay(
    bundle: &DirectRuntimeBundle,
    ble_path: &std::path::Path,
    video_port: u16,
    audio_port: u16,
) -> Result<Child> {
    let video_pipeline = format!("config-interval=1 ! udpsink host=127.0.0.1 port={video_port}");
    let audio_pipeline = format!("pt=96 ! udpsink host=127.0.0.1 port={audio_port}");
    let mut command = Command::new(&bundle.uxplay_path);
    command
        .args([
            "-vs",
            "0",
            "-ble",
            ble_path.to_str().ok_or_else(|| anyhow!("invalid ble path"))?,
            "-vrtp",
            &video_pipeline,
            "-artp",
            &audio_pipeline,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_runtime_env(&mut command, bundle);
    command.spawn().context("failed to spawn uxplay")
}

fn spawn_child(
    bundle_exe: &std::path::Path,
    args: &[String],
    bundle: &DirectRuntimeBundle,
    extra_env: Option<(&str, &std::ffi::OsStr)>,
) -> Result<Child> {
    let mut command = Command::new(bundle_exe);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_runtime_env(&mut command, bundle);
    if let Some((key, value)) = extra_env {
        command.env(key, value);
    }
    command.spawn().with_context(|| format!("failed to spawn {}", bundle_exe.display()))
}

fn apply_runtime_env(command: &mut Command, bundle: &DirectRuntimeBundle) {
    let gst_root = bundle.root.join("gstreamer");
    if cfg!(target_os = "windows") {
        let gst_bin = gst_root.join("bin");
        if gst_bin.is_dir() {
            let path = std::env::var_os("PATH").unwrap_or_default();
            let mut composed = gst_bin.into_os_string();
            if !path.is_empty() {
                composed.push(";");
                composed.push(path);
            }
            command.env("PATH", composed);
        }
        let gst_plugins = gst_root.join("plugins");
        if gst_plugins.is_dir() {
            command.env("GST_PLUGIN_PATH_1_0", gst_plugins);
        }
    } else {
        let gst_plugins = gst_root.join("plugins");
        if gst_plugins.is_dir() {
            command.env("GST_PLUGIN_PATH", gst_plugins);
        }
        let gst_lib = gst_root.join("lib");
        if gst_lib.is_dir() {
            command.env("LD_LIBRARY_PATH", gst_lib);
        }
    }
}

fn kill_child(child: &mut Child) -> Result<()> {
    if child.try_wait()?.is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    Ok(())
}
