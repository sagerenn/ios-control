use std::path::{Path, PathBuf};
use std::process::Command;

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
    let mut base = match std::env::var_os("CARGO_TARGET_DIR") {
        Some(path) => {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                workspace_root.join(path)
            }
        }
        None => workspace_root.join("target"),
    };

    if let Some(target) = std::env::var_os("CARGO_BUILD_TARGET") {
        base.push(target);
    }

    base
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
        .expect("failed to invoke cargo build for local mock plugins");

    assert!(
        output.status.success(),
        "cargo build for local mock plugins failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn local_mock_e2e_builds_streaming_session() {
    let root = workspace_root();
    build_plugins(&root);

    let mut orchestrator = SessionOrchestrator::default();
    let state = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "device-e2e".into(),
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

    // Keep this smoke test pinned to the developer flow documented in README.
    assert_eq!(state.summary.phase, SessionPhase::Streaming);
    assert_eq!(state.summary.capture_plugin.as_deref(), Some("capture.window"));
    assert_eq!(state.summary.control_plugin.as_deref(), Some("control.ble"));
    assert_eq!(
        state.summary.grounding_plugin.as_deref(),
        Some("grounding.core")
    );
    assert_eq!(state.selected_source_id.as_deref(), Some("window-1"));
    assert!(state.latest_frame.is_some());
    assert_eq!(
        state.diagnostics.grounding_summary.as_deref(),
        Some("selected pointer plan")
    );

    state.shutdown().await.unwrap();
}
