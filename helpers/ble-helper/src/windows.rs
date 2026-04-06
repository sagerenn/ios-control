use crate::backend::HostCapability;

pub fn probe_windows_capability() -> HostCapability {
    if let Some(capability) = probe_windows_capability_from_env() {
        return capability;
    }

    let adapter = match windows::Devices::Bluetooth::BluetoothAdapter::GetDefaultAsync()
        .and_then(|operation| operation.join())
    {
        Ok(adapter) => adapter,
        Err(err) => return HostCapability::unsupported(format!("bluetooth radio not detected: {err}")),
    };

    match adapter.IsPeripheralRoleSupported() {
        Ok(true) => HostCapability::supported("windows"),
        Ok(false) => HostCapability::unsupported("bluetooth peripheral role not supported"),
        Err(err) => HostCapability::unsupported(format!(
            "failed to query bluetooth peripheral role: {err}"
        )),
    }
}

fn probe_windows_capability_from_env() -> Option<HostCapability> {
    let radio_present = std::env::var("IOS_CONTROL_BLE_TEST_RADIO")
        .ok()
        .map(|value| value == "1");
    let peripheral_role_supported = std::env::var("IOS_CONTROL_BLE_TEST_PERIPHERAL_ROLE")
        .ok()
        .map(|value| value == "1");

    if radio_present.is_none() && peripheral_role_supported.is_none() {
        return None;
    }

    if !radio_present.unwrap_or(true) {
        return Some(HostCapability::unsupported("bluetooth radio not detected"));
    }
    if !peripheral_role_supported.unwrap_or(true) {
        return Some(HostCapability::unsupported("bluetooth peripheral role not supported"));
    }

    Some(HostCapability::supported("windows"))
}
