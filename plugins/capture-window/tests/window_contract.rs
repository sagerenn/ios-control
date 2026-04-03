use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use plugin_capture_window::helper_bridge::{
    read_next_frame_event, run_probe, HelperFrameEvent, HelperProbe,
};
use plugin_capture_window::helper_config::WindowHelperConfig;
use plugin_capture_window::helper_config::WINDOW_HELPER_SOURCE_ID;
use plugin_capture_window::linux_backend::probe_linux_capture;
use plugin_capture_window::mock_backend::MockWindowBackend;
use plugin_capture_window::windows_backend::probe_windows_capture;
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

struct PluginProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl PluginProcess {
    #[cfg(unix)]
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_plugin-capture-window"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn plugin-capture-window");
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
fn write_helper_script(contents: &str) -> HelperFixture {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("window-helper.sh");
    std::fs::write(&path, contents).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
    HelperFixture { _dir: dir, path }
}

#[tokio::test]
async fn window_capture_lists_mock_source_then_streams_one_frame() {
    let mut backend = MockWindowBackend::default();
    let sources = backend.list_sources().await.unwrap();

    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].source_id, "window:mock");

    let frame = backend.next_frame("window:mock").await.unwrap();
    assert_eq!(frame.frame_index, 1);
    assert_eq!(frame.width, 1280);
}

#[test]
fn window_capture_probe_reports_helper_backed_bridge_support() {
    let helper = tempfile::NamedTempFile::new().unwrap();
    let config = WindowHelperConfig::from_parts(
        Some(helper.path().to_path_buf()),
        Some("Operator Mirror".into()),
    )
    .unwrap();

    let capability = config.capture_capability();
    assert!(capability.available);
    assert_eq!(capability.backend_id, "capture.window.helper");
    assert!(capability.supports_input_bridge);
}

#[test]
fn window_helper_probe_requires_display_name_and_bridge_support() {
    let probe: HelperProbe = serde_json::from_str(
        r#"{"available":true,"display_name":"Operator Mirror","supports_input_bridge":true}"#,
    )
    .unwrap();

    assert!(probe.available);
    assert_eq!(probe.display_name, "Operator Mirror");
    assert!(probe.supports_input_bridge);
}

#[test]
fn window_helper_frame_event_roundtrips_frame_metadata() {
    let event: HelperFrameEvent =
        serde_json::from_str(r#"{"frame_index":7,"width":1280,"height":720,"fill_byte":42}"#)
            .unwrap();

    assert_eq!(event.frame_index, 7);
    assert_eq!(event.width, 1280);
    assert_eq!(event.height, 720);
    assert_eq!(event.fill_byte, 42);
}

#[test]
fn linux_capture_probe_requires_helper_configuration() {
    let _guard = ENV_LOCK.lock().unwrap();
    let old_helper = env::var_os("IOS_CONTROL_WINDOW_CAPTURE_HELPER");
    env::remove_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER");
    let helper = tempfile::NamedTempFile::new().unwrap();

    if cfg!(target_os = "linux") {
        assert!(
            !probe_linux_capture(),
            "default test environment should not claim real capture support"
        );
        env::set_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER", helper.path());
        assert!(probe_linux_capture());
    } else {
        assert!(!probe_linux_capture());
    }

    match old_helper {
        Some(value) => env::set_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER", value),
        None => env::remove_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER"),
    }
}

#[test]
fn windows_capture_probe_requires_helper_configuration() {
    let _guard = ENV_LOCK.lock().unwrap();
    let old_helper = env::var_os("IOS_CONTROL_WINDOW_CAPTURE_HELPER");
    env::remove_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER");
    let helper = tempfile::NamedTempFile::new().unwrap();

    if cfg!(target_os = "windows") {
        assert!(
            !probe_windows_capture(),
            "default test environment should not claim real capture support"
        );
        env::set_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER", helper.path());
        assert!(probe_windows_capture());
    } else {
        assert!(!probe_windows_capture());
    }

    match old_helper {
        Some(value) => env::set_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER", value),
        None => env::remove_var("IOS_CONTROL_WINDOW_CAPTURE_HELPER"),
    }
}

#[cfg(unix)]
#[test]
fn window_helper_unavailable_hides_sources_and_rejects_stream_open() {
    let _guard = ENV_LOCK.lock().unwrap();
    let helper = write_helper_script(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  echo '{"available":false,"display_name":"Mirror Offline","supports_input_bridge":true}'
  exit 0
fi
if [ "$1" = "stream" ]; then
  echo '{"frame_index":1,"width":1280,"height":720,"fill_byte":5}'
  exit 0
fi
exit 2
"#,
    );
    let _env_guard = EnvVarGuard::set("IOS_CONTROL_WINDOW_CAPTURE_HELPER", &helper.path);
    let mut plugin = PluginProcess::spawn();

    plugin.send(HostToPlugin::Handshake {
        protocol_version: 3,
    });
    assert!(matches!(plugin.recv(), PluginToHost::HandshakeAck { .. }));

    plugin.send(HostToPlugin::ProbeCapture);
    match plugin.recv() {
        PluginToHost::CaptureCapability { capability } => {
            assert!(!capability.available);
        }
        other => panic!("unexpected probe reply: {other:?}"),
    }

    plugin.send(HostToPlugin::ListCaptureSources);
    match plugin.recv() {
        PluginToHost::CaptureSources { sources } => assert!(sources.is_empty()),
        other => panic!("unexpected sources reply: {other:?}"),
    }

    plugin.send(HostToPlugin::OpenCaptureStream {
        source_id: WINDOW_HELPER_SOURCE_ID.into(),
    });
    match plugin.recv() {
        PluginToHost::Error { message } => {
            assert_eq!(message, "window capture helper unavailable");
        }
        other => panic!("unexpected open reply: {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn window_read_frame_rejects_helper_geometry_mismatch() {
    let _guard = ENV_LOCK.lock().unwrap();
    let helper = write_helper_script(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  echo '{"available":true,"display_name":"Operator Mirror","supports_input_bridge":true}'
  exit 0
fi
if [ "$1" = "stream" ]; then
  echo '{"frame_index":9,"width":640,"height":480,"fill_byte":7}'
  exit 0
fi
exit 2
"#,
    );
    let _env_guard = EnvVarGuard::set("IOS_CONTROL_WINDOW_CAPTURE_HELPER", &helper.path);
    let mut plugin = PluginProcess::spawn();

    plugin.send(HostToPlugin::Handshake {
        protocol_version: 3,
    });
    assert!(matches!(plugin.recv(), PluginToHost::HandshakeAck { .. }));

    plugin.send(HostToPlugin::OpenCaptureStream {
        source_id: WINDOW_HELPER_SOURCE_ID.into(),
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
fn window_helper_probe_times_out_when_helper_hangs() {
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
fn window_helper_stream_read_times_out_when_helper_hangs() {
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
    let err = read_next_frame_event(&helper.path, WINDOW_HELPER_SOURCE_ID).unwrap_err();
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
fn window_stream_reads_keep_frame_index_monotonic_when_helper_repeats() {
    let _guard = ENV_LOCK.lock().unwrap();
    let helper = write_helper_script(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  echo '{"available":true,"display_name":"Operator Mirror","supports_input_bridge":true}'
  exit 0
fi
if [ "$1" = "stream" ]; then
  echo '{"frame_index":1,"width":1280,"height":720,"fill_byte":7}'
  exit 0
fi
exit 2
"#,
    );
    let _env_guard = EnvVarGuard::set("IOS_CONTROL_WINDOW_CAPTURE_HELPER", &helper.path);
    let mut plugin = PluginProcess::spawn();

    plugin.send(HostToPlugin::Handshake {
        protocol_version: 3,
    });
    assert!(matches!(plugin.recv(), PluginToHost::HandshakeAck { .. }));

    plugin.send(HostToPlugin::OpenCaptureStream {
        source_id: WINDOW_HELPER_SOURCE_ID.into(),
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
fn window_probe_handles_large_stdout_without_timeout() {
    let helper = write_helper_script(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  padding="$(head -c 70000 /dev/zero | tr '\0' 'a')"
  printf '{"available":true,"display_name":"%s","supports_input_bridge":true}\n' "$padding"
  exit 0
fi
exit 2
"#,
    );

    let started = Instant::now();
    let probe = run_probe(&helper.path).unwrap();
    assert!(probe.available);
    assert!(probe.display_name.len() >= 70000);
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "probe path took too long: {:?}",
        started.elapsed()
    );
}
