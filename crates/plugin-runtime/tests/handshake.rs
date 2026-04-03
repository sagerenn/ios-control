use std::path::{Path, PathBuf};
use std::process::Command;

use ios_control_plugin_runtime::PluginRuntime;

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

fn build_plugin_mock_device(workspace_root: &Path) {
    let output = Command::new("cargo")
        .args(["build", "-p", "plugin-mock-device"])
        .current_dir(workspace_root)
        .output()
        .expect("failed to invoke cargo build for plugin-mock-device");

    assert!(
        output.status.success(),
        "cargo build -p plugin-mock-device failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn handshake_returns_mock_plugin_descriptor() {
    let runtime = PluginRuntime::new();
    let workspace_root = workspace_root();
    build_plugin_mock_device(&workspace_root);

    let plugin_path = target_dir(&workspace_root).join(format!(
        "debug/plugin-mock-device{}",
        std::env::consts::EXE_SUFFIX
    ));

    let descriptor = runtime.handshake(&plugin_path).await.unwrap();

    assert_eq!(descriptor.plugin_id, "mock.device");
    assert_eq!(descriptor.protocol_version, 1);
    assert_eq!(descriptor.kind, ios_control_plugin_protocol::PluginKind::Control);
    assert_eq!(descriptor.display_name, "Mock Device");
}
