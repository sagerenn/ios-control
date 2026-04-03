use std::path::{Path, PathBuf};
use std::process::Command;

use ios_control_plugin_protocol::{HostToPlugin, PluginKind, PluginToHost};
use ios_control_plugin_runtime::RunningPlugin;
use serde_json::json;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

fn target_dir(workspace_root: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(path) => {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                workspace_root.join(path)
            }
        }
        None => workspace_root.join("target"),
    }
}

fn build_plugin(workspace_root: &Path, package: &str) {
    let output = Command::new("cargo")
        .args(["build", "-p", package])
        .current_dir(workspace_root)
        .output()
        .expect("failed to invoke cargo build for plugin package");

    assert!(
        output.status.success(),
        "cargo build -p {package} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn plugin_path(workspace_root: &Path, name: &str) -> PathBuf {
    target_dir(workspace_root).join(format!("debug/{}{}", name, std::env::consts::EXE_SUFFIX))
}

struct EnvVarGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let original = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(value) = self.original.take() {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

#[tokio::test]
async fn plugin_runtime_roundtrips_with_real_plugins() {
    let workspace_root = workspace_root();
    for package in [
        "plugin-control-ble",
        "plugin-capture-window",
        "plugin-capture-direct",
        "plugin-grounding-core",
    ] {
        build_plugin(&workspace_root, package);
    }

    let control_path = plugin_path(&workspace_root, "plugin-control-ble");
    let mut control = RunningPlugin::spawn(&control_path).await.unwrap();
    control.send(&HostToPlugin::ProbeControl).await.unwrap();
    match control.read().await.unwrap() {
        PluginToHost::Error { message } => {
            assert_eq!(message, "handshake required for control plugin");
        }
        other => panic!("unexpected control pre-handshake response: {other:?}"),
    }
    let descriptor = control.handshake().await.unwrap();
    assert_eq!(descriptor.plugin_id, "control.ble");
    assert_eq!(descriptor.protocol_version, 2);
    assert_eq!(descriptor.kind, PluginKind::Control);
    assert_eq!(descriptor.display_name, "Bluetooth Control");
    control.send(&HostToPlugin::ProbeControl).await.unwrap();
    match control.read().await.unwrap() {
        PluginToHost::ControlCapability { capability } => {
            if !capability.supported {
                assert!(capability.reason.is_some());
            }
        }
        other => panic!("unexpected control response: {other:?}"),
    };
    control.send(&HostToPlugin::PrepareControl).await.unwrap();
    match control.read().await.unwrap() {
        PluginToHost::ControlSession { checklist, .. } => {
            assert!(!checklist.items.is_empty());
        }
        other => panic!("unexpected control prepare response: {other:?}"),
    }
    control.stop().await.unwrap();

    let window_path = plugin_path(&workspace_root, "plugin-capture-window");
    let _display_guard = EnvVarGuard::set("DISPLAY", ":99");
    let mut window = RunningPlugin::spawn(&window_path).await.unwrap();
    window
        .send(&HostToPlugin::ListCaptureSources)
        .await
        .unwrap();
    match window.read().await.unwrap() {
        PluginToHost::Error { message } => {
            assert_eq!(message, "handshake required for capture-window plugin");
        }
        other => panic!("unexpected capture-window pre-handshake response: {other:?}"),
    }
    let descriptor = window.handshake().await.unwrap();
    assert_eq!(descriptor.plugin_id, "capture.window");
    assert_eq!(descriptor.protocol_version, 2);
    assert_eq!(descriptor.kind, PluginKind::Capture);
    assert_eq!(descriptor.display_name, "Window Capture");
    window
        .send(&HostToPlugin::ListCaptureSources)
        .await
        .unwrap();
    let source_id = match window.read().await.unwrap() {
        PluginToHost::CaptureSources { sources } => {
            assert_eq!(sources.len(), 1);
            sources[0].source_id.clone()
        }
        other => panic!("unexpected capture-window sources response: {other:?}"),
    };
    window
        .send(&HostToPlugin::GetCaptureFrame { source_id })
        .await
        .unwrap();
    match window.read().await.unwrap() {
        PluginToHost::CaptureFrame { frame } => {
            assert_eq!(frame.source_id, "window-1");
        }
        other => panic!("unexpected capture-window frame response: {other:?}"),
    }
    window.stop().await.unwrap();

    let direct_path = plugin_path(&workspace_root, "plugin-capture-direct");
    let _helper_guard = EnvVarGuard::set("IOS_CONTROL_DIRECT_RECEIVER_HELPER", &direct_path);
    let mut direct = RunningPlugin::spawn(&direct_path).await.unwrap();
    direct
        .send(&HostToPlugin::StartDirectCapture)
        .await
        .unwrap();
    match direct.read().await.unwrap() {
        PluginToHost::Error { message } => {
            assert_eq!(message, "handshake required for capture-direct plugin");
        }
        other => panic!("unexpected capture-direct pre-handshake response: {other:?}"),
    }
    let descriptor = direct.handshake().await.unwrap();
    assert_eq!(descriptor.plugin_id, "capture.direct");
    assert_eq!(descriptor.protocol_version, 2);
    assert_eq!(descriptor.kind, PluginKind::Capture);
    assert_eq!(descriptor.display_name, "Direct Receiver");

    direct
        .send(&HostToPlugin::OpenCaptureStream {
            source_id: "direct-1".into(),
        })
        .await
        .unwrap();
    match direct.read().await.unwrap() {
        PluginToHost::CaptureStreamOpened { stream } => {
            assert_eq!(stream.source_id, "direct-1");
            assert!(stream.slot_bytes > 0);
        }
        other => panic!("unexpected capture-direct stream-open response: {other:?}"),
    }
    direct.send(&HostToPlugin::ReadCaptureFrame).await.unwrap();
    match direct.read().await.unwrap() {
        PluginToHost::CaptureFrame { frame } => {
            assert_eq!(frame.source_id, "direct-1");
        }
        other => panic!("unexpected capture-direct read response: {other:?}"),
    }
    direct.send(&HostToPlugin::CloseCaptureStream).await.unwrap();
    match direct.read().await.unwrap() {
        PluginToHost::Ack => {}
        other => panic!("unexpected capture-direct close response: {other:?}"),
    }
    direct.stop().await.unwrap();

    let grounding_path = plugin_path(&workspace_root, "plugin-grounding-core");
    let mut grounding = RunningPlugin::spawn(&grounding_path).await.unwrap();
    let pre_handshake_request: HostToPlugin = serde_json::from_value(json!({
        "PlanGrounding": {
            "request": {
                "target": {
                    "semantic_label": "submit",
                    "visual_region": [120, 240, 360, 480],
                    "confidence": 0.8
                },
                "device_size": [1920, 1080],
                "pointer_estimate": [0.4, 0.6],
                "uncertainty_radius": 0.2,
                "focus_confidence": 0.7,
                "keyboard_preferred": true
            }
        }
    }))
    .unwrap();
    grounding.send(&pre_handshake_request).await.unwrap();
    match grounding.read().await.unwrap() {
        PluginToHost::Error { message } => {
            assert_eq!(message, "handshake required for grounding plugin");
        }
        other => panic!("unexpected grounding pre-handshake response: {other:?}"),
    }
    let descriptor = grounding.handshake().await.unwrap();
    assert_eq!(descriptor.plugin_id, "grounding.core");
    assert_eq!(descriptor.protocol_version, 2);
    assert_eq!(descriptor.kind, PluginKind::Grounding);
    assert_eq!(descriptor.display_name, "Grounding Core");
    let message: HostToPlugin = serde_json::from_value(json!({
        "PlanGrounding": {
            "request": {
                "target": {
                    "semantic_label": "submit",
                    "visual_region": [120, 240, 360, 480],
                    "confidence": 0.8
                },
                "device_size": [1920, 1080],
                "pointer_estimate": [0.4, 0.6],
                "uncertainty_radius": 0.2,
                "focus_confidence": 0.7,
                "keyboard_preferred": true
            }
        }
    }))
    .unwrap();
    grounding.send(&message).await.unwrap();
    match grounding.read().await.unwrap() {
        PluginToHost::GroundingPlan { plan } => {
            assert_eq!(plan.kind.as_str(), "keyboard");
        }
        other => panic!("unexpected grounding response: {other:?}"),
    }
    grounding.stop().await.unwrap();
}
