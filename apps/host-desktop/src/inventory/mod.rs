use ios_control_session_orchestrator::PluginPaths;

use crate::preferences::HostPreferences;

pub mod aggregator;
pub mod model;
pub mod providers;

use aggregator::aggregate_inventory;
use model::InventorySnapshot;
use providers::bluetooth::discover_bluetooth_devices;
use providers::known_devices::discover_known_devices;
use providers::mirror::discover_mirror_devices;

pub fn collect_inventory_snapshot(
    plugin_paths: &PluginPaths,
    preferences: &HostPreferences,
) -> InventorySnapshot {
    let mut observations = Vec::new();
    observations.extend(discover_bluetooth_devices(plugin_paths, preferences));
    observations.extend(discover_mirror_devices(plugin_paths, preferences));
    observations.extend(discover_known_devices(preferences));
    aggregate_inventory(observations)
}
