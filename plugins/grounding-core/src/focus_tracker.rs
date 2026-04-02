#[derive(Debug, Clone, Default)]
pub struct FocusTracker {
    pub focus_confidence: f32,
    pub keyboard_friendly: bool,
}

impl FocusTracker {
    pub fn prefers_keyboard(&self) -> bool {
        self.keyboard_friendly && self.focus_confidence >= 0.7
    }
}
