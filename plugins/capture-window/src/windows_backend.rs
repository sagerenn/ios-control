pub fn probe_windows_capture() -> bool {
    if !cfg!(target_os = "windows") {
        return false;
    }
    std::env::var_os("SESSIONNAME").is_some()
}
