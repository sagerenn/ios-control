use egui::Ui;

use crate::panels::session_view::{self, SessionAction};
use crate::view_models::fleet::FleetViewModel;
use crate::view_models::session::SessionViewModel;

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
) -> (LauncherAction, SessionAction) {
    let mut action = LauncherAction::None;
    let session_action = SessionAction::None;

    ui.heading("Devices");
    if fleet.rows.is_empty() {
        ui.label("No paired Bluetooth devices");
        return (action, session_action);
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

    (action, session_action)
}

pub fn render_with_session_menu(
    ui: &mut Ui,
    fleet: &FleetViewModel,
    selected_device_id: Option<&str>,
    session_menu_device_id: Option<&str>,
    session: &SessionViewModel,
) -> (LauncherAction, SessionAction) {
    let mut launcher_action = LauncherAction::None;
    let mut session_action = SessionAction::None;

    ui.heading("Devices");
    if fleet.rows.is_empty() {
        ui.label("No paired Bluetooth devices");
        return (launcher_action, session_action);
    }

    let selected_visible_device_id = selected_device_id.filter(|device_id| {
        fleet
            .rows
            .iter()
            .any(|row| row.device_id.as_str() == *device_id)
    });
    let menu_row_device_id = if fleet
        .rows
        .iter()
        .any(|row| Some(row.device_id.as_str()) == session_menu_device_id)
    {
        session_menu_device_id
    } else {
        selected_visible_device_id.or_else(|| fleet.rows.first().map(|row| row.device_id.as_str()))
    };

    for row in &fleet.rows {
        ui.horizontal(|ui| {
            let show_menu = menu_row_device_id == Some(row.device_id.as_str());
            let menu_width = if show_menu { 140.0 } else { 0.0 };
            let label_width = (ui.available_width() - menu_width).max(120.0);
            let response = ui.add_sized(
                [label_width, ui.spacing().interact_size.y],
                egui::SelectableLabel::new(
                    Some(row.device_id.as_str()) == selected_device_id,
                    format!("{} | {}", row.device_name, row.readiness_summary),
                ),
            );
            if response.double_clicked() {
                launcher_action = LauncherAction::OpenDevice(row.device_id.clone());
            } else if response.clicked() {
                launcher_action = LauncherAction::SelectDevice(row.device_id.clone());
            }

            if show_menu {
                let action = session_view::render_controls_menu(ui, session);
                if !matches!(action, SessionAction::None) {
                    session_action = action;
                }
            }
        });
    }

    (launcher_action, session_action)
}
