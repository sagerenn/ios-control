pub fn probe_linux_capture() -> bool {
    cfg!(target_os = "linux")
}
