use egui::Ui;

use crate::view_models::fleet::FleetViewModel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LauncherAction {
    None,
    SelectDevice(String),
    OpenDevice(String),
}

pub fn render(
    ui: &mut Ui,
    fleet: &FleetViewModel,
    selected_device_id: Option<&str>,
) -> LauncherAction {
    let mut action = LauncherAction::None;

    ui.heading("Devices");
    if fleet.rows.is_empty() {
        ui.label("No paired Bluetooth devices");
        return action;
    }

    for row in &fleet.rows {
        let response = ui.selectable_label(
            Some(row.device_id.as_str()) == selected_device_id,
            format!("{} | {}", row.device_name, row.readiness_summary),
        );
        if response.double_clicked() {
            action = LauncherAction::OpenDevice(row.device_id.clone());
        } else if response.clicked() {
            action = LauncherAction::SelectDevice(row.device_id.clone());
        }
    }

    action
}
