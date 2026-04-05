use std::collections::BTreeMap;
use std::path::PathBuf;

use ios_control_session_orchestrator::PluginPaths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeLayoutKind {
    Workspace,
    Bundle,
}

#[derive(Debug, Clone)]
pub struct RuntimeLayout {
    pub kind: RuntimeLayoutKind,
    pub root: PathBuf,
    pub plugin_paths: PluginPaths,
    pub helper_paths: BTreeMap<String, PathBuf>,
}
