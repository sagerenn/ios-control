use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use ios_control_session_orchestrator::PluginPaths;

use crate::bootstrap::model::{RuntimeLayout, RuntimeLayoutKind};

#[derive(Debug, Clone)]
pub struct RuntimeLocatorInput {
    pub executable_path: PathBuf,
    pub manifest_dir: PathBuf,
    pub cargo_target_dir: Option<PathBuf>,
    pub cargo_build_target: Option<PathBuf>,
}

pub fn locate_runtime_layout(input: RuntimeLocatorInput) -> Result<RuntimeLayout> {
    let executable_dir = input
        .executable_path
        .parent()
        .ok_or_else(|| anyhow!("executable path has no parent"))?
        .to_path_buf();

    if executable_dir
        .file_name()
        .is_some_and(|name| name == std::ffi::OsStr::new("bin"))
    {
        let root = executable_dir
            .parent()
            .ok_or_else(|| anyhow!("bundle root should exist"))?
            .to_path_buf();
        let bundle_target = root
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("ios-control-"))
            .map(str::to_string);
        let direct_runtime_root = runtime_root_for_target(&root, bundle_target.as_deref());
        let mut helper_paths = std::collections::BTreeMap::new();
        helper_paths.insert("direct_runtime_root".into(), direct_runtime_root.clone());
        return Ok(RuntimeLayout {
            kind: RuntimeLayoutKind::Bundle,
            root: root.clone(),
            plugin_paths: plugin_paths_for_dir(&root.join("plugins"), Some(direct_runtime_root)),
            helper_paths,
        });
    }

    let workspace_root = input
        .manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| anyhow!("workspace root should exist"))?
        .to_path_buf();
    let mut target_dir = match input.cargo_target_dir {
        Some(path) if path.is_absolute() => path,
        Some(path) => workspace_root.join(path),
        None => workspace_root.join("target"),
    };
    if let Some(target) = input.cargo_build_target.as_ref() {
        target_dir.push(target);
    }

    let runtime_target = input
        .cargo_build_target
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned());
    let direct_runtime_root = runtime_root_for_target(&workspace_root, runtime_target.as_deref());
    let mut helper_paths = std::collections::BTreeMap::new();
    helper_paths.insert("direct_runtime_root".into(), direct_runtime_root.clone());

    Ok(RuntimeLayout {
        kind: RuntimeLayoutKind::Workspace,
        root: workspace_root,
        plugin_paths: plugin_paths_for_dir(&target_dir.join("debug"), Some(direct_runtime_root)),
        helper_paths,
    })
}

fn plugin_paths_for_dir(dir: &Path, direct_runtime_root: Option<PathBuf>) -> PluginPaths {
    PluginPaths {
        capture: dir.join(format!("plugin-capture-window{}", std::env::consts::EXE_SUFFIX)),
        capture_direct: dir.join(format!(
            "plugin-capture-direct{}",
            std::env::consts::EXE_SUFFIX
        )),
        capture_direct_runtime_root: direct_runtime_root,
        control_ble: dir.join(format!("plugin-control-ble{}", std::env::consts::EXE_SUFFIX)),
        control_fallback: dir.join(format!(
            "plugin-control-window-bridge{}",
            std::env::consts::EXE_SUFFIX
        )),
        grounding: Some(dir.join(format!(
            "plugin-grounding-core{}",
            std::env::consts::EXE_SUFFIX
        ))),
    }
}

fn runtime_root_for_target(root: &Path, target: Option<&str>) -> PathBuf {
    let target = target
        .map(str::to_string)
        .unwrap_or_else(default_runtime_target);
    root.join("runtime").join("uxplay").join(target)
}

fn default_runtime_target() -> String {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu".into(),
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu".into(),
        ("x86_64", "windows") => "x86_64-pc-windows-msvc".into(),
        ("aarch64", "windows") => "aarch64-pc-windows-msvc".into(),
        (arch, os) => format!("{arch}-{os}"),
    }
}
