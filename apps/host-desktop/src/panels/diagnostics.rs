use egui::Ui;

pub fn render(ui: &mut Ui, message: &str) {
    ui.heading("Diagnostics");
    ui.label(message);
}

pub fn render_control_diagnostics(ui: &mut Ui, message: &str) {
    ui.heading("Control Diagnostics");
    ui.label(message);
}
