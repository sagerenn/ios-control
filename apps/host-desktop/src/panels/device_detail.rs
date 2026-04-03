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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSourceOption {
    pub source_id: String,
    pub display_name: String,
}

impl CaptureSourceOption {
    pub fn new(source_id: &str, display_name: &str) -> Self {
        Self {
            source_id: source_id.into(),
            display_name: display_name.into(),
        }
    }

    pub fn label(&self) -> String {
        if self.source_id.starts_with("window:") {
            format!("Window: {}", self.display_name)
        } else {
            format!("Direct: {}", self.display_name)
        }
    }
}

pub fn render(
    ui: &mut Ui,
    device_name: &str,
    capture_sources: &[CaptureSourceOption],
    control_checklist: &ControlSetupChecklist,
) {
    ui.heading("Device Detail");
    ui.label(device_name);
    for source in capture_sources {
        ui.label(source.label());
    }
    for item in &control_checklist.items {
        ui.label(item);
    }
}
