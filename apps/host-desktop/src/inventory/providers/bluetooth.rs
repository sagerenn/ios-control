use std::process::Command;

use serde::Deserialize;

use ios_control_session_orchestrator::PluginPaths;

use crate::inventory::model::{CapabilityState, DeviceObservation, InventoryEvidenceSource};
use crate::inventory::providers::probe_control_capability;
use crate::preferences::HostPreferences;

#[derive(Debug, Clone, Deserialize)]
struct BluetoothDeviceRecord {
    stable_id: String,
    display_name: String,
}

pub fn discover_bluetooth_devices(
    plugin_paths: &PluginPaths,
    preferences: &HostPreferences,
) -> Vec<DeviceObservation> {
    let devices = test_override_devices().or_else(discover_windows_devices);
    let Some(devices) = devices else {
        return Vec::new();
    };

    let preferred_control_state = map_control_state(probe_control_capability(&plugin_paths.control_ble).ok());

    devices
        .into_iter()
        .map(|device| DeviceObservation {
            provider: InventoryEvidenceSource::Bluetooth,
            stable_id: Some(device.stable_id.clone()),
            known_device_id: preferences
                .known_devices
                .iter()
                .find(|known| known.stable_id.as_deref() == Some(device.stable_id.as_str()))
                .map(|known| known.known_device_id.clone()),
            display_name: device.display_name,
            mirror_source_id: None,
            live: true,
            capture_state: CapabilityState::Unavailable,
            preferred_control_state: preferred_control_state.clone(),
            fallback_control_state: CapabilityState::Unavailable,
            reasons: vec!["paired over bluetooth".into()],
        })
        .collect()
}

fn map_control_state(capability: Option<ios_control_contracts::control::ControlCapability>) -> CapabilityState {
    match capability {
        Some(capability) if capability.supported => CapabilityState::Ready,
        Some(capability) => CapabilityState::Blocked(
            capability
                .reason
                .unwrap_or_else(|| "preferred control unavailable".into()),
        ),
        None => CapabilityState::Unavailable,
    }
}

fn test_override_devices() -> Option<Vec<BluetoothDeviceRecord>> {
    let value = std::env::var("IOS_CONTROL_TEST_BLUETOOTH_DEVICES_JSON").ok()?;
    serde_json::from_str(&value).ok()
}

fn discover_windows_devices() -> Option<Vec<BluetoothDeviceRecord>> {
    if !cfg!(target_os = "windows") {
        return None;
    }

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-PnpDevice -Class Bluetooth -PresentOnly | \
             Where-Object { $_.FriendlyName -match 'iPhone|iPad' } | \
             Select-Object @{Name='stable_id';Expression={$_.InstanceId}}, @{Name='display_name';Expression={$_.FriendlyName}} | \
             ConvertTo-Json -Compress",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    if stdout.trim().is_empty() {
        return Some(Vec::new());
    }

    if stdout.trim_start().starts_with('[') {
        serde_json::from_str(&stdout).ok()
    } else {
        serde_json::from_str::<BluetoothDeviceRecord>(&stdout)
            .ok()
            .map(|device| vec![device])
    }
}
