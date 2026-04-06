use crate::backend::HostCapability;
use std::path::Path;
use zbus::blocking::fdo::DBusProxy;
use zbus::names::BusName;

pub fn probe_linux_capability() -> HostCapability {
    if let Some(capability) = probe_linux_capability_from_env() {
        return capability;
    }

    if !Path::new("/var/run/dbus/system_bus_socket").exists() {
        return HostCapability::unsupported("system bus socket missing");
    }

    let connection = match zbus::blocking::Connection::system() {
        Ok(connection) => connection,
        Err(err) => return HostCapability::unsupported(format!("system bus unavailable: {err}")),
    };

    let dbus = match DBusProxy::new(&connection) {
        Ok(proxy) => proxy,
        Err(err) => {
            return HostCapability::unsupported(format!("dbus proxy unavailable: {err}"));
        }
    };

    let bluez_name = match BusName::try_from("org.bluez") {
        Ok(name) => name,
        Err(err) => return HostCapability::unsupported(format!("invalid bluez bus name: {err}")),
    };

    match dbus.name_has_owner(bluez_name) {
        Ok(true) => {}
        Ok(false) => return HostCapability::unsupported("org.bluez not available"),
        Err(err) => {
            return HostCapability::unsupported(format!("bluez owner query failed: {err}"));
        }
    }

    let adapter_present = std::fs::read_dir("/sys/class/bluetooth")
        .ok()
        .and_then(|mut entries| entries.next())
        .is_some();
    if !adapter_present {
        return HostCapability::unsupported("bluetooth adapter not detected");
    }

    HostCapability::supported("linux")
}

fn probe_linux_capability_from_env() -> Option<HostCapability> {
    let system_bus_socket = std::env::var("IOS_CONTROL_BLE_TEST_SYSTEM_BUS")
        .ok()
        .map(|value| value == "1");
    let adapter_present = std::env::var("IOS_CONTROL_BLE_TEST_ADAPTER")
        .ok()
        .map(|value| value == "1");
    let advertising_supported = std::env::var("IOS_CONTROL_BLE_TEST_ADVERTISING")
        .ok()
        .map(|value| value == "1");

    if system_bus_socket.is_none() && adapter_present.is_none() && advertising_supported.is_none() {
        return None;
    }

    if !system_bus_socket.unwrap_or(true) {
        return Some(HostCapability::unsupported("system bus socket missing"));
    }
    if !adapter_present.unwrap_or(true) {
        return Some(HostCapability::unsupported("bluetooth adapter not detected"));
    }
    if !advertising_supported.unwrap_or(true) {
        return Some(HostCapability::unsupported(
            "bluetooth adapter does not support BLE advertising",
        ));
    }

    Some(HostCapability::supported("linux"))
}
