use std::path::PathBuf;

use host_desktop::bootstrap::model::RuntimeLayoutKind;
use host_desktop::bootstrap::runtime_locator::{locate_runtime_layout, RuntimeLocatorInput};

mod support;
use support::{stage_bundle_layout, target_dir, workspace_root};

#[test]
fn locator_prefers_bundle_layout_from_executable_path() {
    let staged = stage_bundle_layout();

    let layout = locate_runtime_layout(RuntimeLocatorInput {
        executable_path: staged.host_exe.clone(),
        manifest_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        cargo_target_dir: None,
        cargo_build_target: None,
    })
    .expect("bundle layout should resolve");

    assert_eq!(layout.kind, RuntimeLayoutKind::Bundle);
    assert_eq!(
        layout.plugin_paths.capture,
        staged
            .plugins_dir
            .join(format!("plugin-capture-window{}", std::env::consts::EXE_SUFFIX))
    );
    assert_eq!(
        layout.plugin_paths.control_ble,
        staged
            .plugins_dir
            .join(format!("plugin-control-ble{}", std::env::consts::EXE_SUFFIX))
    );
}

#[test]
fn locator_uses_workspace_target_layout_for_repo_launches() {
    let root = workspace_root();
    let exe = target_dir(&root).join(format!("debug/host-desktop{}", std::env::consts::EXE_SUFFIX));

    let layout = locate_runtime_layout(RuntimeLocatorInput {
        executable_path: exe,
        manifest_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        cargo_target_dir: std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from),
        cargo_build_target: std::env::var_os("CARGO_BUILD_TARGET").map(PathBuf::from),
    })
    .expect("workspace layout should resolve");

    assert_eq!(layout.kind, RuntimeLayoutKind::Workspace);
    assert_eq!(
        layout.plugin_paths.control_fallback,
        target_dir(&root).join(format!(
            "debug/plugin-control-window-bridge{}",
            std::env::consts::EXE_SUFFIX
        ))
    );
}
