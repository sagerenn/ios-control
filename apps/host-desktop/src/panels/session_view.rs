use egui::{vec2, TextureHandle, Ui, Vec2};
use ios_control_contracts::capture::VideoFrameDescriptor;
use ios_control_contracts::control::ControlInputEvent;

use crate::preview::PreviewInputBridge;
use crate::view_models::session::{SessionUiState, SessionViewModel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionAction {
    None,
    Start,
    Stop,
    ControlInput(Vec<ControlInputEvent>),
}

pub fn render(
    ui: &mut Ui,
    view_model: &SessionViewModel,
    texture: Option<&TextureHandle>,
    input_bridge: &mut PreviewInputBridge,
) -> SessionAction {
    let mut action = SessionAction::None;

    if !matches!(view_model.ui_state, SessionUiState::Streaming) {
        input_bridge.reset();
    }

    ui.columns(2, |columns| {
        let (left, right) = columns.split_at_mut(1);
        render_session_controls(&mut left[0], view_model, &mut action);

        let events = render_preview(&mut right[0], view_model, texture, input_bridge);
        if !events.is_empty() {
            action = SessionAction::ControlInput(events);
        }
    });

    action
}

fn render_session_controls(ui: &mut Ui, view_model: &SessionViewModel, action: &mut SessionAction) {
    ui.heading("Session View");
    ui.label(view_model.status_line());
    if let Some(detail) = view_model.status_detail.as_ref() {
        ui.label(detail);
    }

    ui.horizontal(|ui| {
        if ui
            .add_enabled(view_model.can_start(), egui::Button::new("Start Session"))
            .clicked()
        {
            *action = SessionAction::Start;
        }
        if ui
            .add_enabled(view_model.can_stop(), egui::Button::new("Stop Session"))
            .clicked()
        {
            *action = SessionAction::Stop;
        }
    });

    if let Some(source) = &view_model.selected_source {
        ui.label(format!("Source: {}", source.label()));
    }

    match &view_model.ui_state {
        SessionUiState::Streaming => {
            if let Some(frame) = &view_model.latest_frame {
                render_frame_summary(ui, frame);
            }
        }
        SessionUiState::Starting => {
            ui.label("Waiting for runtime session status");
        }
        SessionUiState::WaitingForMirror
        | SessionUiState::Idle
        | SessionUiState::Blocked(_)
        | SessionUiState::Error(_) => {}
    }
}

fn render_preview(
    ui: &mut Ui,
    view_model: &SessionViewModel,
    texture: Option<&TextureHandle>,
    input_bridge: &mut PreviewInputBridge,
) -> Vec<ControlInputEvent> {
    let (Some(texture), SessionUiState::Streaming, Some(frame)) = (
        texture,
        &view_model.ui_state,
        view_model.latest_frame.as_ref(),
    ) else {
        input_bridge.reset();
        return Vec::new();
    };

    let preview_size = fitted_preview_size(
        frame,
        ui.available_size_before_wrap(),
        ui.ctx().pixels_per_point().max(1.0),
    );
    let response = ui.add(
        egui::Image::new(texture)
            .fit_to_exact_size(preview_size)
            .sense(egui::Sense::click_and_drag()),
    );
    if response.clicked() || response.drag_started() {
        response.request_focus();
    }
    if input_bridge.is_armed() && !response.hovered() {
        return input_bridge.release_control();
    }
    input_bridge.collect(
        ui.ctx(),
        &response,
        [frame.width.max(1), frame.height.max(1)],
    )
}

fn render_frame_summary(ui: &mut Ui, frame: &VideoFrameDescriptor) {
    ui.label(format!(
        "{}x{} | {}° | {:?} | frame {}",
        frame.width, frame.height, frame.rotation_degrees, frame.health, frame.frame_index
    ));
}

fn fitted_preview_size(
    frame: &VideoFrameDescriptor,
    available: Vec2,
    pixels_per_point: f32,
) -> Vec2 {
    let pixels_per_point = pixels_per_point.max(1.0);
    let frame_size = vec2(
        frame.width.max(1) as f32 / pixels_per_point,
        frame.height.max(1) as f32 / pixels_per_point,
    );
    let available = vec2(available.x.max(1.0), available.y.max(1.0));
    let scale = (available.x / frame_size.x)
        .min(available.y / frame_size.y)
        .min(1.0);

    vec2(
        (frame_size.x * scale).max(1.0),
        (frame_size.y * scale).max(1.0),
    )
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
                render(
                    ui,
                    &view_model,
                    Some(&texture),
                    &mut PreviewInputBridge::default(),
                );
            });
        });

        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect_text(&clipped.shape, &mut texts);
        }
        assert!(texts.iter().any(|text| text.contains(expected)));
    }

    #[test]
    fn session_view_shows_waiting_detail_for_direct_receiver_target() {
        let view_model = SessionViewModel::waiting_for_mirror(Some(CaptureSourceOption::new(
            "direct-1",
            "Direct Receiver",
        )))
        .with_status_detail(Some(
            "Waiting for iPhone screen mirroring to iOS Control 0424".into(),
        ));
        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render(ui, &view_model, None, &mut PreviewInputBridge::default());
            });
        });

        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect_text(&clipped.shape, &mut texts);
        }
        assert!(texts.iter().any(|text| text.contains("iOS Control 0424")));
    }

    #[test]
    fn session_preview_fits_portrait_frame_inside_available_space() {
        let frame = VideoFrameDescriptor {
            source_id: "direct-1".into(),
            source_kind: SourceKind::DirectReceiver,
            width: 1179,
            height: 2556,
            rotation_degrees: 0,
            frame_index: 8,
            health: FrameHealth::Healthy,
        };

        let size = fitted_preview_size(&frame, egui::vec2(900.0, 520.0), 1.0);

        assert!(size.x <= 900.0);
        assert!(size.y <= 520.0);
        assert!((size.x / size.y - 1179.0 / 2556.0).abs() < 0.001);
    }

    #[test]
    fn session_preview_does_not_upscale_past_native_physical_pixels() {
        let frame = VideoFrameDescriptor {
            source_id: "direct-1".into(),
            source_kind: SourceKind::DirectReceiver,
            width: 608,
            height: 1080,
            rotation_degrees: 0,
            frame_index: 8,
            health: FrameHealth::Healthy,
        };

        let size = fitted_preview_size(&frame, egui::vec2(900.0, 900.0), 1.5);

        assert_eq!(size, egui::vec2(608.0 / 1.5, 1080.0 / 1.5));
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
