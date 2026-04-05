use egui::Ui;

use crate::view_models::device_detail::DeviceDetailViewModel;

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
        if self.source_id.starts_with("window:") || self.source_id.starts_with("window-") {
            format!("Window: {}", self.display_name)
        } else {
            format!("Direct: {}", self.display_name)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceDetailAction {
    None,
    SelectCaptureSource(String),
}

pub fn render(ui: &mut Ui, view_model: &DeviceDetailViewModel) -> DeviceDetailAction {
    let mut action = DeviceDetailAction::None;

    ui.heading("Device Detail");
    ui.label(&view_model.device_name);
    for source in &view_model.capture_sources {
        let selected = view_model.active_source_id.as_deref() == Some(source.source_id.as_str());
        if ui.selectable_label(selected, source.label()).clicked() {
            action = DeviceDetailAction::SelectCaptureSource(source.source_id.clone());
        }
    }
    for note in &view_model.inventory_notes {
        ui.label(note);
    }
    for item in &view_model.control_checklist.items {
        ui.label(item);
    }

    action
}
