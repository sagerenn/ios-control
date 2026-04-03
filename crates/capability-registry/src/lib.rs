use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySnapshot {
    pub supported: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CapabilityRegistry {
    entries: BTreeMap<String, CapabilitySnapshot>,
}

impl CapabilityRegistry {
    pub fn record(
        &mut self,
        key: impl Into<String>,
        supported: bool,
        reason: Option<String>,
    ) -> Option<CapabilitySnapshot> {
        self.entries
            .insert(key.into(), CapabilitySnapshot { supported, reason })
    }

    pub fn get(&self, key: &str) -> Option<&CapabilitySnapshot> {
        self.entries.get(key)
    }

    pub fn entries(&self) -> &BTreeMap<String, CapabilitySnapshot> {
        &self.entries
    }
}
