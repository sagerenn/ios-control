pub fn probe_windows_capture() -> bool {
    if !cfg!(target_os = "windows") {
        return false;
    }
    crate::helper_config::resolve_window_capture_helper().is_some()
}
