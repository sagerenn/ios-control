pub fn probe_linux_capture() -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }
    std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some()
}
