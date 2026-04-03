use std::path::PathBuf;
use std::process::{Command, ExitStatus};

pub fn find_helper() -> Option<PathBuf> {
    std::env::var_os("IOS_CONTROL_WINDOW_INPUT_HELPER")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

pub fn helper_available(path: Option<PathBuf>) -> bool {
    path.is_some()
}

pub fn launch_helper(helper: PathBuf, args: &[String]) -> std::io::Result<ExitStatus> {
    Command::new(helper).args(args).status()
}
