use egui::Ui;

use crate::view_models::dashboard::DashboardViewModel;
use crate::view_models::fleet::FleetViewModel;

pub fn render(
    ui: &mut Ui,
    view_model: &DashboardViewModel,
    fleet: &FleetViewModel,
    selected_device_id: Option<&str>,
) -> Option<String> {
    let mut selected = None;

    ui.heading("Dashboard");
    ui.label(format!("Devices: {}", view_model.total_devices));
    ui.label(format!("Degraded: {}", view_model.degraded_devices));

    for row in &fleet.rows {
        let mut label = format!(
            "{} | {} | {}",
            row.device_name,
            row.evidence_badges.join(", "),
            row.readiness_summary
        );
        if row.operator_action.is_some() {
            label.push_str(" | action required");
        }
        if row.active_session {
            label.push_str(" | active");
        }
        if Some(row.device_id.as_str()) == selected_device_id {
            label.push_str(" | selected");
        }
        if ui.button(label).clicked() {
            selected = Some(row.device_id.clone());
        }
    }

    selected
}
