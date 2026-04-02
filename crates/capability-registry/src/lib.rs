use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct CapabilityRegistry {
    entries: BTreeMap<String, bool>,
}

impl CapabilityRegistry {
    pub fn record(&mut self, key: impl Into<String>, value: bool) {
        self.entries.insert(key.into(), value);
    }
}
