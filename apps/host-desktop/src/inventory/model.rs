#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InventoryEvidenceSource {
    Bluetooth,
    Mirror,
    Known,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityState {
    Unavailable,
    Discovered,
    Ready,
    Blocked(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sessionability {
    NotStartable,
    StartableWithPreferredPath,
    StartableWithFallback,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceObservation {
    pub provider: InventoryEvidenceSource,
    pub stable_id: Option<String>,
    pub known_device_id: Option<String>,
    pub display_name: String,
    pub mirror_source_id: Option<String>,
    pub live: bool,
    pub capture_state: CapabilityState,
    pub preferred_control_state: CapabilityState,
    pub fallback_control_state: CapabilityState,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryDevice {
    pub inventory_id: String,
    pub display_name: String,
    pub stable_id: Option<String>,
    pub known_device_id: Option<String>,
    pub mirror_source_id: Option<String>,
    pub live: bool,
    pub evidence_sources: Vec<InventoryEvidenceSource>,
    pub capture_state: CapabilityState,
    pub preferred_control_state: CapabilityState,
    pub fallback_control_state: CapabilityState,
    pub sessionability: Sessionability,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InventorySnapshot {
    pub devices: Vec<InventoryDevice>,
}
