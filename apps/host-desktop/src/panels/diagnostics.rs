use egui::Ui;

pub fn render(ui: &mut Ui, message: &str) {
    ui.heading("Diagnostics");
    ui.label(message);
}
