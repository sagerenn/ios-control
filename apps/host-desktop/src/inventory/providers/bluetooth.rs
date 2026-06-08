use std::collections::BTreeMap;
use std::process::Command;

use serde::Deserialize;

use ios_control_session_orchestrator::PluginPaths;

use crate::inventory::model::{CapabilityState, DeviceObservation, InventoryEvidenceSource};
use crate::inventory::providers::probe_control_capability;
use crate::preferences::{HostPreferences, KnownDevicePreference};

#[derive(Debug, Clone, Deserialize)]
struct BluetoothDeviceRecord {
    stable_id: String,
    display_name: String,
    #[serde(default)]
    container_id: Option<String>,
}

#[derive(Debug, Clone)]
struct LogicalBluetoothDevice {
    stable_id: String,
    display_name: String,
    raw_stable_ids: Vec<String>,
}

pub fn discover_bluetooth_devices(
    plugin_paths: &PluginPaths,
    preferences: &HostPreferences,
) -> Vec<DeviceObservation> {
    let devices = test_override_devices()
        .or_else(discover_windows_devices)
        .map(collapse_windows_records);
    let Some(devices) = devices else {
        return Vec::new();
    };

    let preferred_control_state =
        map_control_state(probe_control_capability(&plugin_paths.control_ble).ok());

    devices
        .into_iter()
        .map(|device| DeviceObservation {
            provider: InventoryEvidenceSource::Bluetooth,
            stable_id: Some(device.stable_id.clone()),
            known_device_id: preferences
                .known_devices
                .iter()
                .find(|known| known_device_matches(known, &device))
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

fn map_control_state(
    capability: Option<ios_control_contracts::control::ControlCapability>,
) -> CapabilityState {
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

fn collapse_windows_records(records: Vec<BluetoothDeviceRecord>) -> Vec<LogicalBluetoothDevice> {
    let mut grouped: BTreeMap<String, Vec<BluetoothDeviceRecord>> = BTreeMap::new();

    for record in records {
        let key = record
            .container_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| record.stable_id.clone());
        grouped.entry(key).or_default().push(record);
    }

    grouped
        .into_iter()
        .map(|(stable_id, records)| LogicalBluetoothDevice {
            stable_id,
            display_name: choose_display_name(&records),
            raw_stable_ids: records
                .iter()
                .map(|record| record.stable_id.clone())
                .collect(),
        })
        .collect()
}

fn choose_display_name(records: &[BluetoothDeviceRecord]) -> String {
    records
        .iter()
        .map(|record| record.display_name.as_str())
        .min_by_key(|name| {
            let cleaned = strip_aux_bluetooth_suffix(name);
            (u8::from(cleaned != *name), cleaned.len(), cleaned)
        })
        .map(|name| strip_aux_bluetooth_suffix(name).to_string())
        .unwrap_or_default()
}

fn strip_aux_bluetooth_suffix(name: &str) -> &str {
    const AUX_SUFFIXES: &[&str] = &[
        " AVRCP Transport",
        " Hands-Free AG Audio",
        " Headset",
        " Stereo",
    ];

    for suffix in AUX_SUFFIXES {
        if let Some(base) = name.strip_suffix(suffix) {
            return base.trim_end();
        }
    }

    name
}

fn known_device_matches(known: &KnownDevicePreference, device: &LogicalBluetoothDevice) -> bool {
    let Some(known_stable_id) = known.stable_id.as_deref() else {
        return false;
    };

    known_stable_id == device.stable_id
        || device
            .raw_stable_ids
            .iter()
            .any(|stable_id| stable_id == known_stable_id)
}

fn discover_windows_devices() -> Option<Vec<BluetoothDeviceRecord>> {
    if !cfg!(target_os = "windows") {
        return None;
    }

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            r#"Get-PnpDevice -Class Bluetooth -PresentOnly |
             Where-Object { $_.FriendlyName -match 'iPhone|iPad' } |
             ForEach-Object {
                 $container = Get-PnpDeviceProperty -InstanceId $_.InstanceId -KeyName 'DEVPKEY_Device_ContainerId' -ErrorAction SilentlyContinue |
                     Select-Object -ExpandProperty Data -ErrorAction SilentlyContinue
                 [pscustomobject]@{
                     stable_id = $_.InstanceId
                     display_name = $_.FriendlyName
                     container_id = [string]$container
                 }
             } |
             ConvertTo-Json -Compress"#,
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
