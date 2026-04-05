use egui::Ui;

use crate::view_models::startup::StartupViewModel;

pub fn render(ui: &mut Ui, view_model: &StartupViewModel) {
    ui.heading("Startup Readiness");
    ui.label(&view_model.summary);
    for item in &view_model.items {
        ui.label(format!("{} | {} | {}", item.label, item.status, item.detail));
    }
}
