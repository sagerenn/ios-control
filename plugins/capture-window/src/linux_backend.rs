pub fn probe_linux_capture() -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }
    std::env::var_os("IOS_CONTROL_WINDOW_CAPTURE_HELPER")
        .map(std::path::PathBuf::from)
        .is_some_and(|path| path.is_file())
}
