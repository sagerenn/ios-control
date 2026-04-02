use egui::Ui;

pub fn render(ui: &mut Ui, device_name: &str) {
    ui.heading("Device Detail");
    ui.label(device_name);
}
