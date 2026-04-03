use egui::Ui;

pub fn render_rows(ui: &mut Ui, rows: &[String]) {
    ui.heading("Settings");
    for row in rows {
        ui.label(row);
    }
}
