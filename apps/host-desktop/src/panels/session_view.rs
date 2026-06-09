use egui::{vec2, Align, Layout, TextureHandle, Ui, Vec2};
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

    let events = render_preview(ui, view_model, texture, input_bridge);
    if !events.is_empty() {
        action = SessionAction::ControlInput(events);
    }

    action
}

pub fn render_controls_menu(ui: &mut Ui, view_model: &SessionViewModel) -> SessionAction {
    let mut action = SessionAction::None;
    ui.menu_button("Session Menu", |ui| {
        ui.set_min_width(280.0);
        render_session_controls(ui, view_model, &mut action);
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
            ui.close_menu();
        }
        if ui
            .add_enabled(view_model.can_stop(), egui::Button::new("Stop Session"))
            .clicked()
        {
            *action = SessionAction::Stop;
            ui.close_menu();
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
        ui.allocate_space(preview_available_size(ui));
        return Vec::new();
    };

    let available = preview_available_size(ui);
    let preview_size = fitted_preview_size(frame, available, ui.ctx().pixels_per_point().max(1.0));
    let top_padding = ((available.y - preview_size.y) / 2.0).max(0.0);
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.add_space(top_padding);
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
    })
    .inner
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

pub fn auto_session_inner_size(
    frame: &VideoFrameDescriptor,
    monitor_size: Vec2,
    pixels_per_point: f32,
) -> Vec2 {
    let pixels_per_point = pixels_per_point.max(1.0);
    let monitor_size = vec2(
        (monitor_size.x / pixels_per_point).max(360.0),
        (monitor_size.y / pixels_per_point).max(520.0),
    );
    let max_preview = vec2(
        (monitor_size.x - 80.0).max(240.0),
        (monitor_size.y - 80.0).max(320.0),
    );
    let preview_size = fitted_preview_size(frame, max_preview, pixels_per_point);
    vec2(preview_size.x, preview_size.y)
}

fn preview_available_size(ui: &Ui) -> Vec2 {
    let max_rect = ui.max_rect();
    let cursor_top = ui.cursor().top();
    vec2(
        ui.available_width().max(1.0),
        (max_rect.bottom() - cursor_top).max(1.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panels::device_detail::CaptureSourceOption;
    use egui::{Color32, ColorImage};
    use ios_control_contracts::capture::{FrameHealth, SourceKind};

    #[test]
    fn session_view_renders_mirror_only_without_control_menu() {
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
        assert!(!texts.iter().any(|text| text.contains("Session Menu")));
        assert!(!texts.iter().any(|text| text.contains(expected)));
    }

    #[test]
    fn control_host_session_menu_button_lives_outside_session_window() {
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
                render_controls_menu(ui, &view_model);
            });
        });

        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect_text(&clipped.shape, &mut texts);
        }
        assert!(texts.iter().any(|text| text.contains("Session Menu")));
        assert!(!texts.iter().any(|text| text.contains("iOS Control 0424")));
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
    fn session_preview_respects_remaining_column_height() {
        let frame = VideoFrameDescriptor {
            source_id: "direct-1".into(),
            source_kind: SourceKind::DirectReceiver,
            width: 1080,
            height: 1920,
            rotation_degrees: 0,
            frame_index: 8,
            health: FrameHealth::Healthy,
        };

        let size = fitted_preview_size(&frame, egui::vec2(520.0, 620.0), 1.0);

        assert!(size.x <= 520.0);
        assert!(size.y <= 620.0);
        assert!((size.x / size.y - 1080.0 / 1920.0).abs() < 0.001);
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

    #[test]
    fn session_window_auto_size_matches_phone_aspect_without_side_gutters() {
        let frame = VideoFrameDescriptor {
            source_id: "direct-1".into(),
            source_kind: SourceKind::DirectReceiver,
            width: 1080,
            height: 1920,
            rotation_degrees: 0,
            frame_index: 8,
            health: FrameHealth::Healthy,
        };

        let size = auto_session_inner_size(&frame, egui::vec2(1536.0, 864.0), 1.0);

        assert!(size.x <= 1536.0);
        assert!(size.y <= 864.0);
        assert!((size.x / size.y - 1080.0 / 1920.0).abs() < 0.001);
    }

    #[test]
    fn session_window_auto_size_converts_physical_monitor_pixels_to_points() {
        let frame = VideoFrameDescriptor {
            source_id: "direct-1".into(),
            source_kind: SourceKind::DirectReceiver,
            width: 1080,
            height: 1920,
            rotation_degrees: 0,
            frame_index: 8,
            health: FrameHealth::Healthy,
        };

        let size = auto_session_inner_size(&frame, egui::vec2(1536.0, 864.0), 1.25);

        assert!(size.y <= 864.0 / 1.25);
        assert!((size.x / size.y - 1080.0 / 1920.0).abs() < 0.001);
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
