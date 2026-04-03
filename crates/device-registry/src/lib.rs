use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRecord {
    pub device_id: String,
    pub device_name: String,
    pub preferred_capture_plugin: String,
    pub preferred_control_plugin: String,
    pub preferred_grounding_plugin: Option<String>,
    pub last_source_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DeviceRegistry {
    entries: BTreeMap<String, DeviceRecord>,
}

impl DeviceRegistry {
    pub fn upsert(&mut self, record: DeviceRecord) -> Option<DeviceRecord> {
        self.entries.insert(record.device_id.clone(), record)
    }

    pub fn get(&self, device_id: &str) -> Option<&DeviceRecord> {
        self.entries.get(device_id)
    }

    pub fn entries(&self) -> &BTreeMap<String, DeviceRecord> {
        &self.entries
    }
}
