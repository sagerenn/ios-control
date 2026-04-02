use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct DeviceRegistry {
    names: BTreeMap<String, String>,
}

impl DeviceRegistry {
    pub fn upsert(&mut self, device_id: impl Into<String>, device_name: impl Into<String>) {
        self.names.insert(device_id.into(), device_name.into());
    }
}
