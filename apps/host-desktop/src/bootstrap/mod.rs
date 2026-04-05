use std::path::PathBuf;

use anyhow::Result;

use crate::view_models::startup::StartupViewModel;

pub mod capability_probe;
pub mod model;
pub mod runtime_locator;

use capability_probe::startup_from_plugin_paths;
use model::RuntimeLayout;
use runtime_locator::{locate_runtime_layout, RuntimeLocatorInput};

#[derive(Debug, Clone)]
pub struct HostBootstrap {
    pub layout: RuntimeLayout,
    pub startup: StartupViewModel,
}

pub fn bootstrap_startup(executable_path: PathBuf, manifest_dir: PathBuf) -> Result<HostBootstrap> {
    let layout = locate_runtime_layout(RuntimeLocatorInput {
        executable_path,
        manifest_dir,
        cargo_target_dir: std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from),
        cargo_build_target: std::env::var_os("CARGO_BUILD_TARGET").map(PathBuf::from),
    })?;
    let startup = startup_from_plugin_paths(&layout.plugin_paths);
    Ok(HostBootstrap { layout, startup })
}
