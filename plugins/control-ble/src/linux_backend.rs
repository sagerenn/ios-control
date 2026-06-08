use crate::backend::ControlCapability;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxProbeResult {
    pub supported: bool,
    pub reason: Option<String>,
}

impl LinuxProbeResult {
    pub fn from_service_name(service_name: Option<&str>) -> Self {
        match service_name {
            Some("org.bluez") => Self {
                supported: true,
                reason: None,
            },
            _ => Self {
                supported: false,
                reason: Some("org.bluez not available".into()),
            },
        }
    }

    pub fn from_runtime_checks(
        system_bus_socket: bool,
        adapter_present: bool,
        bluetoothd_present: bool,
    ) -> Self {
        if !system_bus_socket {
            return Self {
                supported: false,
                reason: Some("system bus socket missing".into()),
            };
        }
        if !adapter_present {
            return Self {
                supported: false,
                reason: Some("bluetooth adapter not detected".into()),
            };
        }
        if !bluetoothd_present {
            return Self {
                supported: false,
                reason: Some("bluetooth backend tools not detected".into()),
            };
        }
        Self {
            supported: true,
            reason: None,
        }
    }

    pub fn as_capability(&self) -> ControlCapability {
        ControlCapability {
            supported: self.supported,
            reason: self.reason.clone(),
        }
    }
}

pub fn probe_linux_backend() -> LinuxProbeResult {
    let system_bus_socket = Path::new("/var/run/dbus/system_bus_socket").exists();
    let adapter_present = std::fs::read_dir("/sys/class/bluetooth")
        .ok()
        .and_then(|mut entries| entries.next())
        .is_some();
    let bluetoothd_present = [
        "/usr/sbin/bluetoothd",
        "/usr/lib/bluetooth/bluetoothd",
        "/usr/libexec/bluetooth/bluetoothd",
    ]
    .iter()
    .any(|path| Path::new(path).exists());
    LinuxProbeResult::from_runtime_checks(system_bus_socket, adapter_present, bluetoothd_present)
}
