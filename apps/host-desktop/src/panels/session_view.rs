use egui::Ui;
use ios_control_contracts::capture::VideoFrameDescriptor;

use crate::view_models::session::{SessionUiState, SessionViewModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAction {
    None,
    Start,
    Stop,
}

pub fn render(ui: &mut Ui, view_model: &SessionViewModel) -> SessionAction {
    let mut action = SessionAction::None;

    ui.heading("Session View");
    ui.label(view_model.status_line());

    ui.horizontal(|ui| {
        if ui
            .add_enabled(view_model.can_start(), egui::Button::new("Start Session"))
            .clicked()
        {
            action = SessionAction::Start;
        }
        if ui
            .add_enabled(view_model.can_stop(), egui::Button::new("Stop Session"))
            .clicked()
        {
            action = SessionAction::Stop;
        }
    });

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
            ui.label("Waiting for runtime session status");
        }
        SessionUiState::Idle | SessionUiState::Error(_) => {
            if let Some(source) = &view_model.selected_source {
                ui.label(format!("Source: {}", source.label()));
            }
        }
    }

    action
}

fn render_frame_summary(ui: &mut Ui, frame: &VideoFrameDescriptor) {
    ui.label(format!(
        "{}x{} frame {}",
        frame.width, frame.height, frame.frame_index
    ));
}
