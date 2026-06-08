use ios_control_contracts::capture::{AudioStreamPhase, CaptureStreamPhase, FrameHealth};
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use plugin_capture_direct::helper_bridge::HelperFrameEvent;
use plugin_capture_direct::helper_launcher::{
    capture_capability, read_next_frame_event, run_probe,
};
use std::env;
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvVarGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &std::path::Path) -> Self {
        let original = env::var_os(key);
        env::set_var(key, value);
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(value) = self.original.take() {
            env::set_var(self.key, value);
        } else {
            env::remove_var(self.key);
        }
    }
}

struct EnvStringGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

impl EnvStringGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let original = env::var_os(key);
        env::set_var(key, value);
        Self { key, original }
    }
}

impl Drop for EnvStringGuard {
    fn drop(&mut self) {
        if let Some(value) = self.original.take() {
            env::set_var(self.key, value);
        } else {
            env::remove_var(self.key);
        }
    }
}

struct PluginProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl PluginProcess {
    #[cfg(unix)]
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_plugin-capture-direct"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn plugin-capture-direct");
        let stdin = child.stdin.take().expect("plugin stdin");
        let stdout = BufReader::new(child.stdout.take().expect("plugin stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, request: HostToPlugin) {
        let payload = serde_json::to_string(&request).unwrap();
        self.stdin.write_all(payload.as_bytes()).unwrap();
        self.stdin.write_all(b"\n").unwrap();
        self.stdin.flush().unwrap();
    }

    fn recv(&mut self) -> PluginToHost {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(line.trim()).unwrap()
    }
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(unix)]
struct HelperFixture {
    _dir: tempfile::TempDir,
    path: std::path::PathBuf,
}

#[cfg(unix)]
struct RuntimeBundleFixture {
    _dir: tempfile::TempDir,
    root: std::path::PathBuf,
}

#[cfg(unix)]
impl RuntimeBundleFixture {
    fn path(&self) -> &std::path::Path {
        &self.root
    }
}

#[cfg(unix)]
fn write_helper_script(contents: &str) -> HelperFixture {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("direct-helper.sh");
    std::fs::write(&path, contents).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
    HelperFixture { _dir: dir, path }
}

#[cfg(unix)]
fn write_runtime_bundle_fixture() -> RuntimeBundleFixture {
    let dir = tempfile::tempdir().unwrap();
    let manifest_path = dir.path().join("manifest.json");
    std::fs::write(
        &manifest_path,
        r#"{"uxplay_path":"uxplay","gst_launch_path":"gst-launch-1.0","beacon_helper_path":"beacon-helper","beacon_script_path":"Bluetooth_LE_beacon/uxplay-beacon.py","python_path":"python3"}"#,
    )
    .unwrap();

    let uxplay_path = dir.path().join("uxplay");
    std::fs::write(
        &uxplay_path,
        r#"#!/bin/sh
if [ "$1" = "--probe" ]; then
  exit 0
fi
exit 0
"#,
    )
    .unwrap();
    let mut perms = std::fs::metadata(&uxplay_path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&uxplay_path, perms).unwrap();

    let gst_launch_path = dir.path().join("gst-launch-1.0");
    std::fs::write(
        &gst_launch_path,
        r#"#!/bin/sh
exit 0
"#,
    )
    .unwrap();
    let mut perms = std::fs::metadata(&gst_launch_path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&gst_launch_path, perms).unwrap();

    let beacon_helper_path = dir.path().join("beacon-helper");
    std::fs::write(
        &beacon_helper_path,
        r#"#!/bin/sh
exit 0
"#,
    )
    .unwrap();
    let mut perms = std::fs::metadata(&beacon_helper_path)
        .unwrap()
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&beacon_helper_path, perms).unwrap();

    let beacon_dir = dir.path().join("Bluetooth_LE_beacon");
    std::fs::create_dir_all(&beacon_dir).unwrap();
    std::fs::write(beacon_dir.join("uxplay-beacon.py"), "print('ok')\n").unwrap();

    RuntimeBundleFixture {
        _dir: dir,
        root: manifest_path.parent().unwrap().to_path_buf(),
    }
}

#[cfg(unix)]
fn write_runtime_bundle_fixture_without_receivers() -> RuntimeBundleFixture {
    let dir = tempfile::tempdir().unwrap();
    let manifest_path = dir.path().join("manifest.json");
    std::fs::write(
        &manifest_path,
        r#"{"uxplay_path":"uxplay","gst_launch_path":"gst-launch-1.0","beacon_helper_path":"beacon-helper","beacon_script_path":"Bluetooth_LE_beacon/uxplay-beacon.py","python_path":"python3"}"#,
    )
    .unwrap();

    let uxplay_path = dir.path().join("uxplay");
    std::fs::write(
        &uxplay_path,
        r#"#!/bin/sh
exit 0
"#,
    )
    .unwrap();
    let mut perms = std::fs::metadata(&uxplay_path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&uxplay_path, perms).unwrap();

    RuntimeBundleFixture {
        _dir: dir,
        root: manifest_path.parent().unwrap().to_path_buf(),
    }
}

#[cfg(unix)]
fn write_runtime_bundle_fixture_with_media_scripts() -> RuntimeBundleFixture {
    let dir = tempfile::tempdir().unwrap();
    let manifest_path = dir.path().join("manifest.json");
    std::fs::write(
        &manifest_path,
        r#"{"uxplay_path":"uxplay","gst_launch_path":"gst-launch-1.0","beacon_helper_path":"beacon-helper","beacon_script_path":"Bluetooth_LE_beacon/uxplay-beacon.py","python_path":"python3"}"#,
    )
    .unwrap();

    let uxplay_path = dir.path().join("uxplay");
    std::fs::write(
        &uxplay_path,
        r#"#!/bin/sh
if [ "$1" = "--help" ]; then
  exit 0
fi
while [ $# -gt 0 ]; do
  if [ "$1" = "-ble" ]; then
    shift
    echo "beacon-data" > "$1"
  fi
  shift
done
sleep 60
"#,
    )
    .unwrap();
    let mut perms = std::fs::metadata(&uxplay_path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&uxplay_path, perms).unwrap();

    let gst_launch_path = dir.path().join("gst-launch-1.0");
    std::fs::write(
        &gst_launch_path,
        r#"#!/bin/sh
location=""
if [ -n "$IOS_CONTROL_DIRECT_FRAME_PATTERN" ]; then
  location="$IOS_CONTROL_DIRECT_FRAME_PATTERN"
fi
for arg in "$@"; do
  case "$arg" in
    location=*)
      location="${arg#location=}"
      ;;
  esac
done
if [ -n "$location" ]; then
  output=$(printf "$location" 1)
  mkdir -p "$(dirname "$output")"
  printf '%s' 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==' | base64 -d > "$output"
fi
sleep 60
"#,
    )
    .unwrap();
    let mut perms = std::fs::metadata(&gst_launch_path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&gst_launch_path, perms).unwrap();

    let beacon_helper_path = dir.path().join("beacon-helper");
    std::fs::write(
        &beacon_helper_path,
        r#"#!/bin/sh
sleep 60
"#,
    )
    .unwrap();
    let mut perms = std::fs::metadata(&beacon_helper_path)
        .unwrap()
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&beacon_helper_path, perms).unwrap();

    let beacon_dir = dir.path().join("Bluetooth_LE_beacon");
    std::fs::create_dir_all(&beacon_dir).unwrap();
    std::fs::write(beacon_dir.join("uxplay-beacon.py"), "print('ok')\n").unwrap();

    RuntimeBundleFixture {
        _dir: dir,
        root: manifest_path.parent().unwrap().to_path_buf(),
    }
}

#[cfg(unix)]
fn write_runtime_bundle_fixture_requiring_runtime_env() -> RuntimeBundleFixture {
    let dir = tempfile::tempdir().unwrap();
    let manifest_path = dir.path().join("manifest.json");
    std::fs::write(
        &manifest_path,
        r#"{"uxplay_path":"uxplay","gst_launch_path":"gstreamer/bin/gst-launch-1.0","beacon_helper_path":"beacon-helper","beacon_script_path":"Bluetooth_LE_beacon/uxplay-beacon.py","python_path":"python3"}"#,
    )
    .unwrap();

    let uxplay_path = dir.path().join("uxplay");
    std::fs::write(
        &uxplay_path,
        r#"#!/bin/sh
root="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
expected_lib="$root/gstreamer/lib"
expected_plugins="$root/gstreamer/plugins"
case ":${LD_LIBRARY_PATH:-}:" in
  *":$expected_lib:"*) ;;
  *) exit 11 ;;
esac
case ":${GST_PLUGIN_PATH:-}:" in
  *":$expected_plugins:"*) ;;
  *) exit 12 ;;
esac
exit 0
"#,
    )
    .unwrap();
    let mut perms = std::fs::metadata(&uxplay_path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&uxplay_path, perms).unwrap();

    let gst_bin = dir.path().join("gstreamer").join("bin");
    let gst_lib = dir.path().join("gstreamer").join("lib");
    let gst_plugins = dir.path().join("gstreamer").join("plugins");
    std::fs::create_dir_all(&gst_bin).unwrap();
    std::fs::create_dir_all(&gst_lib).unwrap();
    std::fs::create_dir_all(&gst_plugins).unwrap();
    std::fs::write(gst_bin.join("gst-launch-1.0"), "#!/bin/sh\nexit 0\n").unwrap();
    let mut perms = std::fs::metadata(gst_bin.join("gst-launch-1.0"))
        .unwrap()
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(gst_bin.join("gst-launch-1.0"), perms).unwrap();

    let beacon_helper_path = dir.path().join("beacon-helper");
    std::fs::write(
        &beacon_helper_path,
        r#"#!/bin/sh
exit 0
"#,
    )
    .unwrap();
    let mut perms = std::fs::metadata(&beacon_helper_path)
        .unwrap()
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&beacon_helper_path, perms).unwrap();

    let beacon_dir = dir.path().join("Bluetooth_LE_beacon");
    std::fs::create_dir_all(&beacon_dir).unwrap();
    std::fs::write(beacon_dir.join("uxplay-beacon.py"), "print('ok')\n").unwrap();

    RuntimeBundleFixture {
        _dir: dir,
        root: manifest_path.parent().unwrap().to_path_buf(),
    }
}

#[test]
fn direct_receiver_probe_requires_existing_executable() {
    let capability = capture_capability(None);
    assert!(!capability.available);
    assert_eq!(
        capability.reason.as_deref(),
        Some("IOS_CONTROL_DIRECT_RECEIVER_HELPER not configured")
    );
}

#[cfg(unix)]
#[test]
fn direct_probe_requires_runtime_manifest_and_uxplay_binary() {
    let _guard = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let _runtime_guard = EnvVarGuard::set("IOS_CONTROL_DIRECT_RUNTIME_ROOT", dir.path());

    let capability = capture_capability(None);
    assert!(!capability.available);
    assert!(
        capability
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("manifest"),
        "unexpected reason: {:?}",
        capability.reason
    );
}

#[cfg(unix)]
#[test]
fn direct_probe_accepts_minimal_runtime_bundle_fixture() {
    let _guard = ENV_LOCK.lock().unwrap();
    let fixture = write_runtime_bundle_fixture();
    let _runtime_guard = EnvVarGuard::set("IOS_CONTROL_DIRECT_RUNTIME_ROOT", fixture.path());

    let capability = capture_capability(None);
    assert!(capability.available, "reason: {:?}", capability.reason);
    assert_eq!(capability.backend_id, "capture.direct.uxplay");
}

#[cfg(unix)]
#[test]
fn direct_probe_requires_manifest_receiver_paths() {
    let _guard = ENV_LOCK.lock().unwrap();
    let fixture = write_runtime_bundle_fixture_without_receivers();
    let _runtime_guard = EnvVarGuard::set("IOS_CONTROL_DIRECT_RUNTIME_ROOT", fixture.path());

    let capability = capture_capability(None);
    assert!(!capability.available);
    assert!(
        capability
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("gst-launch")
            || capability
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("beacon"),
        "unexpected reason: {:?}",
        capability.reason
    );
}

#[cfg(unix)]
#[test]
fn direct_probe_uses_runtime_environment_from_bundle() {
    let _guard = ENV_LOCK.lock().unwrap();
    let fixture = write_runtime_bundle_fixture_requiring_runtime_env();
    let _runtime_guard = EnvVarGuard::set("IOS_CONTROL_DIRECT_RUNTIME_ROOT", fixture.path());

    let capability = capture_capability(None);
    assert!(capability.available, "reason: {:?}", capability.reason);
}

#[cfg(unix)]
#[test]
fn capture_status_reports_waiting_before_stream_is_open() {
    let _guard = ENV_LOCK.lock().unwrap();
    let fixture = write_runtime_bundle_fixture();
    let _runtime_guard = EnvVarGuard::set("IOS_CONTROL_DIRECT_RUNTIME_ROOT", fixture.path());
    let mut plugin = PluginProcess::spawn();

    plugin.send(HostToPlugin::Handshake {
        protocol_version: 3,
    });
    assert!(matches!(plugin.recv(), PluginToHost::HandshakeAck { .. }));

    plugin.send(HostToPlugin::GetCaptureStatus);
    match plugin.recv() {
        PluginToHost::CaptureStatus { status } => {
            assert_eq!(status.video_phase, CaptureStreamPhase::Opening);
            assert_eq!(status.audio.phase, AudioStreamPhase::Idle);
        }
        other => panic!("unexpected status reply: {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn direct_runtime_stream_opens_and_reads_generated_frame() {
    let _guard = ENV_LOCK.lock().unwrap();
    let fixture = write_runtime_bundle_fixture_with_media_scripts();
    let _runtime_guard = EnvVarGuard::set("IOS_CONTROL_DIRECT_RUNTIME_ROOT", fixture.path());
    let mut plugin = PluginProcess::spawn();

    plugin.send(HostToPlugin::Handshake {
        protocol_version: 3,
    });
    assert!(matches!(plugin.recv(), PluginToHost::HandshakeAck { .. }));

    plugin.send(HostToPlugin::OpenCaptureStream {
        source_id: "direct-1".into(),
    });
    assert!(matches!(
        plugin.recv(),
        PluginToHost::CaptureStreamOpened { .. }
    ));

    plugin.send(HostToPlugin::ReadCaptureFrame);
    match plugin.recv() {
        PluginToHost::CaptureFrame { frame } => {
            assert_eq!(frame.source_id, "direct-1");
            assert!(frame.frame_index >= 1);
        }
        other => panic!("unexpected read reply: {other:?}"),
    }
}

#[test]
fn direct_helper_frame_event_defaults_rotation_and_health() {
    let event: HelperFrameEvent = serde_json::from_str(
        r#"{"frame_index":3,"width":1179,"height":2556,"rgba_base64":"AP8A/w=="}"#,
    )
    .unwrap();

    assert_eq!(event.frame_index, 3);
    assert_eq!(event.rotation_degrees, 0);
    assert_eq!(event.health, FrameHealth::Healthy);
}

#[test]
fn direct_helper_frame_event_decodes_rgba_payload() {
    let event: HelperFrameEvent =
        serde_json::from_str(r#"{"frame_index":3,"width":2,"height":1,"rgba_base64":"AP8A/w=="}"#)
            .unwrap();

    assert_eq!(event.frame_index, 3);
    assert_eq!(event.decode_rgba().unwrap(), vec![0, 255, 0, 255]);
}

#[test]
fn helper_frame_event_decodes_rgba_rotation_and_health() {
    let event: HelperFrameEvent = serde_json::from_str(
        r#"{
            "frame_index": 4,
            "width": 2,
            "height": 1,
            "rotation_degrees": 90,
            "health": "Occluded",
            "rgba_base64": "AQIDBAUGBwg="
        }"#,
    )
    .unwrap();

    assert_eq!(event.decode_rgba().unwrap(), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(event.rotation_degrees, 90);
    assert_eq!(event.health, FrameHealth::Occluded);
}

#[test]
fn helper_mode_can_delay_first_stream_once_via_env() {
    let _guard = ENV_LOCK.lock().unwrap();
    let state_file = std::env::temp_dir().join(format!(
        "capture-direct-delay-once-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    let _delay_guard =
        EnvStringGuard::set("IOS_CONTROL_DIRECT_HELPER_DELAY_FIRST_STREAM_MS", "2200");
    let _state_guard = EnvStringGuard::set(
        "IOS_CONTROL_DIRECT_HELPER_DELAY_STATE_FILE",
        &state_file.display().to_string(),
    );

    let started = Instant::now();
    let first = Command::new(env!("CARGO_BIN_EXE_plugin-capture-direct"))
        .args(["stream", "--source", "direct-1"])
        .output()
        .expect("first delayed helper stream should spawn");
    let first_elapsed = started.elapsed();

    let second_started = Instant::now();
    let second = Command::new(env!("CARGO_BIN_EXE_plugin-capture-direct"))
        .args(["stream", "--source", "direct-1"])
        .output()
        .expect("second helper stream should spawn");
    let second_elapsed = second_started.elapsed();

    assert!(first.status.success());
    assert!(second.status.success());
    assert!(
        first_elapsed >= Duration::from_millis(2100),
        "first helper stream should delay once, elapsed={first_elapsed:?}"
    );
    assert!(
        second_elapsed < Duration::from_secs(2),
        "second helper stream should return quickly, elapsed={second_elapsed:?}"
    );
}

#[cfg(unix)]
#[test]
fn direct_probe_rejects_incompatible_helper_contract() {
    let helper = write_helper_script(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  echo 'not-json'
  exit 0
fi
exit 2
"#,
    );

    let capability = capture_capability(Some(helper.path.clone()));
    assert!(!capability.available);
    assert!(
        capability
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("incompatible helper probe"),
        "unexpected reason: {:?}",
        capability.reason
    );
}

#[cfg(unix)]
#[test]
fn direct_read_frame_rejects_helper_geometry_mismatch() {
    let _guard = ENV_LOCK.lock().unwrap();
    let helper = write_helper_script(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  echo '{"available":true,"supports_input_bridge":false}'
  exit 0
fi
if [ "$1" = "stream" ]; then
  echo '{"frame_index":11,"width":720,"height":1280,"rotation_degrees":0,"health":"Healthy","rgba_base64":"AQIDBA=="}'
  exit 0
fi
exit 2
"#,
    );
    let _env_guard = EnvVarGuard::set("IOS_CONTROL_DIRECT_RECEIVER_HELPER", &helper.path);
    let mut plugin = PluginProcess::spawn();

    plugin.send(HostToPlugin::Handshake {
        protocol_version: 3,
    });
    assert!(matches!(plugin.recv(), PluginToHost::HandshakeAck { .. }));

    plugin.send(HostToPlugin::OpenCaptureStream {
        source_id: "direct-1".into(),
    });
    assert!(matches!(
        plugin.recv(),
        PluginToHost::CaptureStreamOpened { .. }
    ));

    plugin.send(HostToPlugin::ReadCaptureFrame);
    match plugin.recv() {
        PluginToHost::Error { message } => {
            assert!(
                message.contains("helper frame geometry mismatch"),
                "actual message: {message}"
            );
        }
        other => panic!("unexpected read reply: {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn direct_read_frame_rejects_invalid_rgba_base64_without_crashing() {
    let _guard = ENV_LOCK.lock().unwrap();
    let helper = write_helper_script(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  echo '{"available":true,"supports_input_bridge":false}'
  exit 0
fi
if [ "$1" = "stream" ]; then
  echo '{"frame_index":11,"width":1179,"height":2556,"rotation_degrees":0,"health":"Healthy","rgba_base64":"!!!"}'
  exit 0
fi
exit 2
"#,
    );
    let _env_guard = EnvVarGuard::set("IOS_CONTROL_DIRECT_RECEIVER_HELPER", &helper.path);
    let mut plugin = PluginProcess::spawn();

    plugin.send(HostToPlugin::Handshake {
        protocol_version: 3,
    });
    assert!(matches!(plugin.recv(), PluginToHost::HandshakeAck { .. }));

    plugin.send(HostToPlugin::OpenCaptureStream {
        source_id: "direct-1".into(),
    });
    assert!(matches!(
        plugin.recv(),
        PluginToHost::CaptureStreamOpened { .. }
    ));

    plugin.send(HostToPlugin::ReadCaptureFrame);
    match plugin.recv() {
        PluginToHost::Error { message } => {
            assert!(
                message.contains("failed to decode helper frame payload"),
                "actual message: {message}"
            );
        }
        other => panic!("unexpected read reply: {other:?}"),
    }

    plugin.send(HostToPlugin::CloseCaptureStream);
    assert!(matches!(plugin.recv(), PluginToHost::Ack));
}

#[cfg(unix)]
#[test]
fn direct_helper_probe_times_out_when_helper_hangs() {
    let helper = write_helper_script(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  sleep 5
  exit 0
fi
exit 2
"#,
    );

    let started = Instant::now();
    let err = run_probe(&helper.path).unwrap_err();
    assert!(
        err.to_string().contains("timed out"),
        "unexpected error: {err}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "timeout path took too long: {:?}",
        started.elapsed()
    );
}

#[cfg(unix)]
#[test]
fn direct_helper_stream_read_times_out_when_helper_hangs() {
    let helper = write_helper_script(
        r#"#!/bin/sh
if [ "$1" = "stream" ]; then
  sleep 5
  exit 0
fi
exit 2
"#,
    );

    let started = Instant::now();
    let err = read_next_frame_event(&helper.path, "direct-1").unwrap_err();
    assert!(
        err.to_string().contains("timed out"),
        "unexpected error: {err}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "timeout path took too long: {:?}",
        started.elapsed()
    );
}

#[cfg(unix)]
#[test]
fn direct_stream_reads_keep_frame_index_monotonic_when_helper_repeats() {
    let _guard = ENV_LOCK.lock().unwrap();
    let helper = write_helper_script(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  echo '{"available":true,"supports_input_bridge":false}'
  exit 0
fi
if [ "$1" = "stream" ]; then
  printf '{"frame_index":1,"width":1179,"height":2556,"rotation_degrees":0,"health":"Healthy","rgba_base64":"'
  head -c 12054096 /dev/zero | tr '\0' '\11' | base64 | tr -d '\n'
  printf '"}\n'
  exit 0
fi
exit 2
"#,
    );
    let _env_guard = EnvVarGuard::set("IOS_CONTROL_DIRECT_RECEIVER_HELPER", &helper.path);
    let mut plugin = PluginProcess::spawn();

    plugin.send(HostToPlugin::Handshake {
        protocol_version: 3,
    });
    assert!(matches!(plugin.recv(), PluginToHost::HandshakeAck { .. }));

    plugin.send(HostToPlugin::OpenCaptureStream {
        source_id: "direct-1".into(),
    });
    assert!(matches!(
        plugin.recv(),
        PluginToHost::CaptureStreamOpened { .. }
    ));

    plugin.send(HostToPlugin::ReadCaptureFrame);
    let first_index = match plugin.recv() {
        PluginToHost::CaptureFrame { frame } => frame.frame_index,
        other => panic!("unexpected first read reply: {other:?}"),
    };
    plugin.send(HostToPlugin::ReadCaptureFrame);
    let second_index = match plugin.recv() {
        PluginToHost::CaptureFrame { frame } => frame.frame_index,
        other => panic!("unexpected second read reply: {other:?}"),
    };

    assert!(second_index > first_index);
    assert_eq!(first_index, 1);
    assert_eq!(second_index, 2);
}

#[cfg(unix)]
#[test]
fn direct_probe_handles_large_stdout_without_timeout() {
    let helper = write_helper_script(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  padding="$(head -c 70000 /dev/zero | tr '\0' 'a')"
  printf '{"available":true,"supports_input_bridge":false,"padding":"%s"}\n' "$padding"
  exit 0
fi
exit 2
"#,
    );

    let started = Instant::now();
    let probe = run_probe(&helper.path).unwrap();
    assert!(probe.available);
    assert!(!probe.supports_input_bridge);
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "probe path took too long: {:?}",
        started.elapsed()
    );
}

#[cfg(unix)]
#[test]
fn start_direct_capture_requires_configured_helper() {
    let _guard = ENV_LOCK.lock().unwrap();
    let old_helper = env::var_os("IOS_CONTROL_DIRECT_RECEIVER_HELPER");
    env::remove_var("IOS_CONTROL_DIRECT_RECEIVER_HELPER");
    let mut plugin = PluginProcess::spawn();

    plugin.send(HostToPlugin::Handshake {
        protocol_version: 3,
    });
    assert!(matches!(plugin.recv(), PluginToHost::HandshakeAck { .. }));

    plugin.send(HostToPlugin::StartDirectCapture);
    match plugin.recv() {
        PluginToHost::Error { message } => {
            assert_eq!(message, "direct receiver helper not configured");
        }
        other => panic!("unexpected start capture reply: {other:?}"),
    }

    match old_helper {
        Some(value) => env::set_var("IOS_CONTROL_DIRECT_RECEIVER_HELPER", value),
        None => env::remove_var("IOS_CONTROL_DIRECT_RECEIVER_HELPER"),
    }
}

#[cfg(unix)]
#[test]
fn start_direct_capture_rejects_incompatible_helper() {
    let _guard = ENV_LOCK.lock().unwrap();
    let helper = write_helper_script(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  echo 'not-json'
  exit 0
fi
exit 2
"#,
    );
    let _env_guard = EnvVarGuard::set("IOS_CONTROL_DIRECT_RECEIVER_HELPER", &helper.path);
    let mut plugin = PluginProcess::spawn();

    plugin.send(HostToPlugin::Handshake {
        protocol_version: 3,
    });
    assert!(matches!(plugin.recv(), PluginToHost::HandshakeAck { .. }));

    plugin.send(HostToPlugin::StartDirectCapture);
    match plugin.recv() {
        PluginToHost::Error { message } => {
            assert!(
                message.contains("incompatible direct helper probe"),
                "unexpected message: {message}"
            );
        }
        other => panic!("unexpected start capture reply: {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn start_direct_capture_rejects_helper_payload_size_mismatch() {
    let _guard = ENV_LOCK.lock().unwrap();
    let helper = write_helper_script(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  echo '{"available":true,"supports_input_bridge":false}'
  exit 0
fi
if [ "$1" = "stream" ]; then
  echo '{"frame_index":2,"width":1179,"height":2556,"rotation_degrees":0,"health":"Healthy","rgba_base64":"AQIDBA=="}'
  exit 0
fi
exit 2
"#,
    );
    let _env_guard = EnvVarGuard::set("IOS_CONTROL_DIRECT_RECEIVER_HELPER", &helper.path);
    let mut plugin = PluginProcess::spawn();

    plugin.send(HostToPlugin::Handshake {
        protocol_version: 3,
    });
    assert!(matches!(plugin.recv(), PluginToHost::HandshakeAck { .. }));

    plugin.send(HostToPlugin::StartDirectCapture);
    match plugin.recv() {
        PluginToHost::Error { message } => {
            assert!(
                message.contains("helper frame payload size mismatch"),
                "unexpected message: {message}"
            );
        }
        other => panic!("unexpected start capture reply: {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn start_direct_capture_rejects_invalid_rgba_base64_without_crashing() {
    let _guard = ENV_LOCK.lock().unwrap();
    let helper = write_helper_script(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  echo '{"available":true,"supports_input_bridge":false}'
  exit 0
fi
if [ "$1" = "stream" ]; then
  echo '{"frame_index":2,"width":1179,"height":2556,"rotation_degrees":0,"health":"Healthy","rgba_base64":"!!!"}'
  exit 0
fi
exit 2
"#,
    );
    let _env_guard = EnvVarGuard::set("IOS_CONTROL_DIRECT_RECEIVER_HELPER", &helper.path);
    let mut plugin = PluginProcess::spawn();

    plugin.send(HostToPlugin::Handshake {
        protocol_version: 3,
    });
    assert!(matches!(plugin.recv(), PluginToHost::HandshakeAck { .. }));

    plugin.send(HostToPlugin::StartDirectCapture);
    match plugin.recv() {
        PluginToHost::Error { message } => {
            assert!(
                message.contains("failed to decode helper frame payload"),
                "unexpected message: {message}"
            );
        }
        other => panic!("unexpected start capture reply: {other:?}"),
    }

    plugin.send(HostToPlugin::ProbeCapture);
    assert!(matches!(
        plugin.recv(),
        PluginToHost::CaptureCapability { .. }
    ));
}
