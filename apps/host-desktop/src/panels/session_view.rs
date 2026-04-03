use egui::Ui;
use ios_control_contracts::capture::VideoFrameDescriptor;

use crate::view_models::session::SessionViewModel;

pub fn render(ui: &mut Ui, view_model: &SessionViewModel) {
    ui.heading("Session View");
    match &view_model.selected_source {
        Some(source) => {
            ui.label(format!("Source: {}", source.label()));
            if let Some(frame) = &view_model.latest_frame {
                render_frame_summary(ui, frame);
            } else {
                ui.label("Waiting for frames");
            }
        }
        None => {
            ui.label("No active session");
        }
    }
}

fn render_frame_summary(ui: &mut Ui, frame: &VideoFrameDescriptor) {
    ui.label(format!(
        "{}x{} frame {}",
        frame.width, frame.height, frame.frame_index
    ));
}
