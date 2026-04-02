use egui::Ui;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlSetupChecklist {
    pub items: Vec<String>,
}

impl ControlSetupChecklist {
    pub fn for_pointer_mode() -> Self {
        Self {
            items: vec![
                "Enable AssistiveTouch on the iPhone or iPad".into(),
                "Enable Full Keyboard Access for keyboard navigation".into(),
                "Pair the host over Bluetooth".into(),
            ],
        }
    }
}

pub fn render(ui: &mut Ui, device_name: &str) {
    ui.heading("Device Detail");
    ui.label(device_name);
}
