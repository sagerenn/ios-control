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
        return Ok(RuntimeLayout {
            kind: RuntimeLayoutKind::Bundle,
            root: root.clone(),
            plugin_paths: plugin_paths_for_dir(&root.join("plugins")),
            helper_paths: Default::default(),
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
    if let Some(target) = input.cargo_build_target {
        target_dir.push(target);
    }

    Ok(RuntimeLayout {
        kind: RuntimeLayoutKind::Workspace,
        root: workspace_root,
        plugin_paths: plugin_paths_for_dir(&target_dir.join("debug")),
        helper_paths: Default::default(),
    })
}

fn plugin_paths_for_dir(dir: &Path) -> PluginPaths {
    PluginPaths {
        capture: dir.join(format!("plugin-capture-window{}", std::env::consts::EXE_SUFFIX)),
        capture_direct: dir.join(format!(
            "plugin-capture-direct{}",
            std::env::consts::EXE_SUFFIX
        )),
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
