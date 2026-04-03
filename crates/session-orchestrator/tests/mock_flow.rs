use std::path::{Path, PathBuf};
use std::process::Command;

use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::SessionPhase;
use ios_control_session_orchestrator::{PluginPaths, SessionOrchestrator, StartSessionRequest};

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

fn plugin_path(workspace_root: &Path, name: &str) -> PathBuf {
    target_dir(workspace_root).join(format!("debug/{}{}", name, std::env::consts::EXE_SUFFIX))
}

fn build_plugins(workspace_root: &Path) {
    let output = Command::new("cargo")
        .args([
            "build",
            "-p",
            "plugin-capture-window",
            "-p",
            "plugin-control-ble",
            "-p",
            "plugin-grounding-core",
        ])
        .current_dir(workspace_root)
        .output()
        .expect("failed to invoke cargo build for mock plugins");

    assert!(
        output.status.success(),
        "cargo build for mock plugins failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn start_session_collects_mock_plugin_state() {
    let root = workspace_root();
    build_plugins(&root);

    let mut orchestrator = SessionOrchestrator::default();
    let state = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "device-1".into(),
            device_name: "Mock iPhone".into(),
            selected_source_id: Some("window-1".into()),
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                control: plugin_path(&root, "plugin-control-ble"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    assert_eq!(state.summary.device_id, "device-1");
    assert_eq!(state.summary.device_name, "Mock iPhone");
    assert_eq!(state.summary.phase, SessionPhase::Streaming);
    assert_eq!(state.summary.plugin_health, PluginHealth::Healthy);
    assert_eq!(
        state.summary.capture_plugin.as_deref(),
        Some("capture.window")
    );
    assert_eq!(state.summary.control_plugin.as_deref(), Some("control.ble"));
    assert_eq!(
        state.summary.grounding_plugin.as_deref(),
        Some("grounding.core")
    );
    assert_eq!(state.selected_source_id.as_deref(), Some("window-1"));

    assert_eq!(state.capture_sources.len(), 1);
    assert_eq!(state.capture_sources[0].source_id, "window-1");
    assert_eq!(state.latest_frame.as_ref().unwrap().source_id, "window-1");
    assert_eq!(state.control_checklist.items.len(), 2);
    assert!(state
        .control_checklist
        .items
        .iter()
        .any(|item| item.contains("Enable Bluetooth")));
    assert!(state.diagnostics.control_summary.contains("supported"));
    assert!(state
        .diagnostics
        .grounding_summary
        .as_deref()
        .unwrap()
        .contains("selected"));

    let control_capability = orchestrator.capabilities.get("control.ble").unwrap();
    assert!(control_capability.supported);
    assert_eq!(control_capability.reason, None);

    let device = orchestrator.devices.get("device-1").unwrap();
    assert_eq!(device.device_name, "Mock iPhone");
    assert_eq!(device.preferred_capture_plugin, "capture.window");
    assert_eq!(device.preferred_control_plugin, "control.ble");
    assert_eq!(
        device.preferred_grounding_plugin.as_deref(),
        Some("grounding.core")
    );
    assert_eq!(device.last_source_id.as_deref(), Some("window-1"));

    let telemetry = orchestrator.telemetry.for_session("device-1");
    assert!(telemetry
        .iter()
        .any(|event| event.message == "session started"));
    assert!(telemetry
        .iter()
        .any(|event| event.message.contains("capture source")));
    assert!(telemetry
        .iter()
        .any(|event| event.message.contains("grounding planned")));
}
