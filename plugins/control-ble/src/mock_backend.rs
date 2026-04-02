use crate::backend::ControlCapability;

pub fn healthy_capability() -> ControlCapability {
    ControlCapability {
        supported: true,
        reason: None,
    }
}
