use egui::Ui;

use crate::view_models::dashboard::DashboardViewModel;

pub fn render(ui: &mut Ui, view_model: &DashboardViewModel) {
    ui.heading("Dashboard");
    ui.label(format!("Devices: {}", view_model.total_devices));
    ui.label(format!("Degraded: {}", view_model.degraded_devices));
}
