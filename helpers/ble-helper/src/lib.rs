pub mod backend;
#[cfg(target_os = "linux")]
pub mod linux;
pub mod state;
#[cfg(target_os = "windows")]
pub mod windows;

pub fn probe_host_capability() -> backend::HostCapability {
    #[cfg(target_os = "linux")]
    {
        return linux::probe_linux_capability();
    }

    #[cfg(target_os = "windows")]
    {
        return windows::probe_windows_capability();
    }

    #[allow(unreachable_code)]
    backend::HostCapability::unsupported("BLE helper is only supported on Linux and Windows")
}
