use egui::{TextureHandle, Ui};
use ios_control_contracts::capture::VideoFrameDescriptor;

use crate::view_models::session::{SessionUiState, SessionViewModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAction {
    None,
    Start,
    Stop,
}

pub fn render(
    ui: &mut Ui,
    view_model: &SessionViewModel,
    texture: Option<&TextureHandle>,
) -> SessionAction {
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
            if let Some(texture) = texture {
                ui.image(texture);
            }
        }
        SessionUiState::Starting => {
            ui.label("Waiting for runtime session status");
        }
        SessionUiState::WaitingForMirror | SessionUiState::Idle | SessionUiState::Blocked(_) | SessionUiState::Error(_) => {
            if let Some(source) = &view_model.selected_source {
                ui.label(format!("Source: {}", source.label()));
            }
        }
    }

    action
}

fn render_frame_summary(ui: &mut Ui, frame: &VideoFrameDescriptor) {
    ui.label(format!(
        "{}x{} | {}° | {:?} | frame {}",
        frame.width, frame.height, frame.rotation_degrees, frame.health, frame.frame_index
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panels::device_detail::CaptureSourceOption;
    use egui::{Color32, ColorImage};
    use ios_control_contracts::capture::{FrameHealth, SourceKind};

    #[test]
    fn session_view_keeps_frame_metadata_visible_even_with_preview_texture() {
        let frame = VideoFrameDescriptor {
            source_id: "window-helper-1".into(),
            source_kind: SourceKind::Window,
            width: 640,
            height: 360,
            rotation_degrees: 90,
            frame_index: 8,
            health: FrameHealth::Occluded,
        };
        let view_model = SessionViewModel::streaming(
            CaptureSourceOption::new("window-helper-1", "Operator Mirror"),
            frame,
        );
        let expected = "640x360 | 90° | Occluded | frame 8";
        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let texture = ui.ctx().load_texture(
                    "session-view-test",
                    ColorImage::new([1, 1], Color32::WHITE),
                    egui::TextureOptions::LINEAR,
                );
                render(ui, &view_model, Some(&texture));
            });
        });

        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect_text(&clipped.shape, &mut texts);
        }
        assert!(texts.iter().any(|text| text.contains(expected)));
    }

    fn collect_text(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(text) => out.push(text.galley.job.text.clone()),
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect_text(shape, out);
                }
            }
            _ => {}
        }
    }
}
