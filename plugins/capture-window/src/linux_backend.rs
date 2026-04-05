pub fn probe_linux_capture() -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }
    crate::helper_config::resolve_window_capture_helper().is_some()
}
