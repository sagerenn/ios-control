use anyhow::{anyhow, Context, Result};
use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use tempfile::TempDir;

use crate::airplay_mdns::{AirPlayMdnsConfig, AirPlayMdnsPublisher};
use crate::direct_status::DirectCaptureStatus;
use crate::rtp_audio;
use crate::rtp_video::{self, DecodedFrame};
use crate::runtime_bundle::{DirectRuntimeBundle, BLE_PATH_ENV, RUNTIME_ROOT_ENV};

const MIRROR_REQUEST_SIZE: &str = "1080x1920";
const AIRPLAY_PORT_BASE: &str = "52081";
const AIRPLAY_RTSP_PORT: u16 = 52082;
const AIRPLAY_DEVICE_ID_ENV: &str = "IOS_CONTROL_AIRPLAY_DEVICE_ID";
const AIRPLAY_DISPLAY_NAME_ENV: &str = "IOS_CONTROL_AIRPLAY_DISPLAY_NAME";
const DEFAULT_AIRPLAY_DEVICE_ID: &str = "02:49:4F:53:43:54";
const DEFAULT_AIRPLAY_DISPLAY_NAME: &str = "iOS Control";

pub struct DirectRuntimeSession {
    _session_dir: TempDir,
    ble_path: PathBuf,
    video_receiver: Child,
    video_frames: rtp_video::RawFrameReader,
    audio_receiver: Child,
    beacon: Child,
    uxplay: Child,
    mdns: AirPlayMdnsPublisher,
    receiver_name: String,
    last_frame_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AirPlayIdentity {
    device_id: String,
    name: String,
}

impl DirectRuntimeSession {
    pub fn start(bundle: &DirectRuntimeBundle, status: &mut DirectCaptureStatus) -> Result<Self> {
        let session_dir = tempfile::tempdir().context("failed to create direct session dir")?;
        let ble_path = session_dir.path().join("uxplay.ble");
        let video_port = reserve_udp_port()?;
        let audio_port = reserve_udp_port()?;
        let frame_config = rtp_video::LiveFrameConfig::from_env();
        let identity = airplay_identity();

        let (video_receiver, video_frames) = spawn_video_receiver(
            &bundle.gst_launch_path,
            &rtp_video::receiver_args(video_port, frame_config),
            bundle,
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
        let uxplay_stdout_log_path = session_dir.path().join("uxplay.stdout.log");
        let uxplay_stderr_log_path = session_dir.path().join("uxplay.stderr.log");
        let uxplay = spawn_uxplay(
            bundle,
            &ble_path,
            &identity,
            video_port,
            audio_port,
            &uxplay_stdout_log_path,
            &uxplay_stderr_log_path,
        )
        .context("failed to launch uxplay")?;
        let mdns = AirPlayMdnsPublisher::start(AirPlayMdnsConfig {
            receiver_name: identity.name.clone(),
            device_id: identity.device_id.clone(),
            rtsp_port: AIRPLAY_RTSP_PORT,
        });

        status.waiting_for_runtime_frame();
        status.detail = Some(format!(
            "Waiting for iPhone screen mirroring to {}",
            identity.name
        ));
        status.audio_route = ios_control_contracts::capture::AudioRoute::LocalPlayback;
        status.audio_active = true;
        status.audio_phase = ios_control_contracts::capture::AudioStreamPhase::Waiting;

        Ok(Self {
            _session_dir: session_dir,
            ble_path,
            video_receiver,
            video_frames,
            audio_receiver,
            beacon,
            uxplay,
            mdns,
            receiver_name: identity.name,
            last_frame_index: 0,
        })
    }

    pub fn next_frame(&mut self) -> Result<Option<DecodedFrame>> {
        if let Some(frame) = self.video_frames.take_latest_after(self.last_frame_index)? {
            self.last_frame_index = frame.frame_index;
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
            status.detail = Some(format!(
                "Waiting for iPhone screen mirroring to {}",
                self.receiver_name
            ));
        }
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.mdns.shutdown();
        let _ = kill_child(&mut self.uxplay);
        let _ = kill_child(&mut self.beacon);
        let _ = kill_child(&mut self.video_receiver);
        self.video_frames.join();
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
    hide_child_console(&mut command);
    command
        .arg("serve")
        .env(RUNTIME_ROOT_ENV, &bundle.root)
        .env(BLE_PATH_ENV, ble_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    apply_runtime_env(&mut command, bundle);
    command
        .spawn()
        .context("failed to spawn direct beacon helper")
}

fn spawn_uxplay(
    bundle: &DirectRuntimeBundle,
    ble_path: &std::path::Path,
    identity: &AirPlayIdentity,
    video_port: u16,
    audio_port: u16,
    stdout_log_path: &std::path::Path,
    stderr_log_path: &std::path::Path,
) -> Result<Child> {
    let video_pipeline = format!(
        "pt=96 config-interval=1 ! udpsink host=127.0.0.1 port={video_port} sync=false async=false"
    );
    let audio_pipeline =
        format!("pt=96 ! udpsink host=127.0.0.1 port={audio_port} sync=false async=false");
    let stdout_log = std::fs::File::create(stdout_log_path)
        .with_context(|| format!("failed to create {}", stdout_log_path.display()))?;
    let stderr_log = std::fs::File::create(stderr_log_path)
        .with_context(|| format!("failed to create {}", stderr_log_path.display()))?;
    let mut command = Command::new(&bundle.uxplay_path);
    hide_child_console(&mut command);
    command
        .args(uxplay_args(
            identity,
            ble_path,
            &video_pipeline,
            &audio_pipeline,
        )?)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_log))
        .stderr(Stdio::from(stderr_log));
    apply_runtime_env(&mut command, bundle);
    command.spawn().context("failed to spawn uxplay")
}

fn uxplay_args(
    identity: &AirPlayIdentity,
    ble_path: &std::path::Path,
    video_pipeline: &str,
    audio_pipeline: &str,
) -> Result<Vec<String>> {
    Ok(vec![
        "-n".into(),
        identity.name.clone(),
        "-nh".into(),
        "-m".into(),
        identity.device_id.clone(),
        "-s".into(),
        MIRROR_REQUEST_SIZE.into(),
        "-p".into(),
        AIRPLAY_PORT_BASE.into(),
        "-vs".into(),
        "fakesink".into(),
        "-ble".into(),
        ble_path
            .to_str()
            .ok_or_else(|| anyhow!("invalid ble path"))?
            .into(),
        "-vrtp".into(),
        video_pipeline.into(),
        "-artp".into(),
        audio_pipeline.into(),
    ])
}

fn airplay_identity() -> AirPlayIdentity {
    airplay_identity_from_values(
        non_empty_env(AIRPLAY_DEVICE_ID_ENV),
        non_empty_env(AIRPLAY_DISPLAY_NAME_ENV),
    )
}

fn airplay_identity_from_values(
    device_id: Option<String>,
    name: Option<String>,
) -> AirPlayIdentity {
    let device_id = device_id.unwrap_or_else(|| DEFAULT_AIRPLAY_DEVICE_ID.into());
    let name = name.unwrap_or_else(|| DEFAULT_AIRPLAY_DISPLAY_NAME.into());

    AirPlayIdentity { device_id, name }
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn spawn_child(
    bundle_exe: &std::path::Path,
    args: &[String],
    bundle: &DirectRuntimeBundle,
    extra_env: Option<(&str, &std::ffi::OsStr)>,
) -> Result<Child> {
    let mut command = Command::new(bundle_exe);
    hide_child_console(&mut command);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    apply_runtime_env(&mut command, bundle);
    if let Some((key, value)) = extra_env {
        command.env(key, value);
    }
    command
        .spawn()
        .with_context(|| format!("failed to spawn {}", bundle_exe.display()))
}

fn spawn_video_receiver(
    bundle_exe: &std::path::Path,
    args: &[String],
    bundle: &DirectRuntimeBundle,
) -> Result<(Child, rtp_video::RawFrameReader)> {
    let mut command = Command::new(bundle_exe);
    hide_child_console(&mut command);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    apply_runtime_env(&mut command, bundle);
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn {}", bundle_exe.display()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("video receiver missing stdout pipe"))?;
    let frame_config = rtp_video::LiveFrameConfig::from_env();
    let reader = rtp_video::RawFrameReader::start(stdout, frame_config)?;
    Ok((child, reader))
}

fn apply_runtime_env(command: &mut Command, bundle: &DirectRuntimeBundle) {
    bundle.apply_runtime_env(command);
}

fn hide_child_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
}

fn kill_child(child: &mut Child) -> Result<()> {
    if child.try_wait()?.is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        airplay_identity_from_values, uxplay_args, AirPlayIdentity, AIRPLAY_PORT_BASE,
        AIRPLAY_RTSP_PORT, DEFAULT_AIRPLAY_DEVICE_ID, DEFAULT_AIRPLAY_DISPLAY_NAME,
        MIRROR_REQUEST_SIZE,
    };

    #[test]
    fn uxplay_args_keep_receiver_headless_without_aggressive_session_resets() {
        let identity = AirPlayIdentity {
            device_id: "02:11:22:33:44:55".into(),
            name: "iOS Control 4455".into(),
        };
        let args = uxplay_args(
            &identity,
            std::path::Path::new("uxplay.ble"),
            "pt=96 config-interval=1 ! udpsink host=127.0.0.1 port=50000 sync=false async=false",
            "pt=96 ! udpsink host=127.0.0.1 port=50001 sync=false async=false",
        )
        .unwrap();

        assert_eq!(args[0..2], ["-n", "iOS Control 4455"]);
        assert!(args
            .windows(2)
            .any(|pair| pair == ["-m", "02:11:22:33:44:55"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["-s", MIRROR_REQUEST_SIZE]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["-p", AIRPLAY_PORT_BASE]));
        assert_eq!(AIRPLAY_RTSP_PORT, 52082);
        assert!(args.windows(2).any(|pair| pair == ["-vs", "fakesink"]));
        assert!(args.iter().any(|arg| arg == "-vrtp"));
        assert!(args.iter().any(|arg| arg == "-artp"));
        assert!(!args.iter().any(|arg| arg == "-reset"));
        assert!(!args.iter().any(|arg| arg == "-nohold"));
        assert!(!args.iter().any(|arg| arg == "-nofreeze"));
        assert!(!args.iter().any(|arg| arg == "-FPSdata"));
        assert!(!args.windows(2).any(|pair| pair == ["-d", "1"]));
    }

    #[test]
    fn airplay_identity_is_stable_by_default() {
        let identity = airplay_identity_from_values(None, None);

        assert_eq!(identity.device_id, DEFAULT_AIRPLAY_DEVICE_ID);
        assert_eq!(identity.name, DEFAULT_AIRPLAY_DISPLAY_NAME);
    }
}
