use egui::Ui;
use ios_control_contracts::capture::VideoFrameDescriptor;

use crate::view_models::session::{SessionUiState, SessionViewModel};

pub fn render(ui: &mut Ui, view_model: &SessionViewModel) {
    ui.heading("Session View");
    ui.label(view_model.status_line());
    match &view_model.ui_state {
        SessionUiState::Streaming => {
            if let Some(source) = &view_model.selected_source {
                ui.label(format!("Source: {}", source.label()));
            }
            if let Some(frame) = &view_model.latest_frame {
                render_frame_summary(ui, frame);
            }
        }
        SessionUiState::Starting => {
            ui.label("Waiting for frames");
        }
        SessionUiState::Idle | SessionUiState::Error(_) => {
            if let Some(source) = &view_model.selected_source {
                ui.label(format!("Source: {}", source.label()));
            }
        }
    }
}

fn render_frame_summary(ui: &mut Ui, frame: &VideoFrameDescriptor) {
    ui.label(format!(
        "{}x{} frame {}",
        frame.width, frame.height, frame.frame_index
    ));
}
