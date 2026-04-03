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
fn write_helper_script(contents: &str) -> HelperFixture {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("direct-helper.sh");
    std::fs::write(&path, contents).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
    HelperFixture { _dir: dir, path }
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

#[test]
fn direct_helper_frame_event_requires_slot_fill_metadata() {
    let event: HelperFrameEvent =
        serde_json::from_str(r#"{"frame_index":3,"width":1179,"height":2556,"fill_byte":64}"#)
            .unwrap();

    assert_eq!(event.frame_index, 3);
    assert_eq!(event.fill_byte, 64);
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
  echo '{"frame_index":11,"width":720,"height":1280,"fill_byte":9}'
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
