use egui::Ui;
pub fn render_summary(ui: &mut Ui, frame_summary: &str, source_label: &str) {
    ui.heading("Session View");
    ui.label(source_label);
    ui.label(frame_summary);
}
