use std::path::Path;

use anyhow::{Context, Result};
use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, Response};
use ios_control_contracts::capture::{CaptureStreamDescriptor, VideoFrameDescriptor};
use ios_control_contracts::control::{
    ControlInputEvent, KeyModifiers, KeyboardInputReport, MouseInputReport,
};
use ios_control_frame_transport::FrameSlotReader;

const POINTER_EDGE_SETTLE_REPORTS: usize = 3;
const POINTER_TARGET_SETTLE_REPORTS: usize = 3;
pub const DEFAULT_PHONE_POINTER_LONG_AXIS_UNITS: u32 = 120;
pub const DEFAULT_TABLET_POINTER_LONG_AXIS_UNITS: u32 = 160;
const POINTER_LONG_AXIS_UNITS_ENV: &str = "IOS_CONTROL_BLE_POINTER_LONG_AXIS_UNITS";
pub const MIN_POINTER_LONG_AXIS_UNITS: u32 = 60;
pub const MAX_POINTER_LONG_AXIS_UNITS: u32 = 1600;
const TABLET_ASPECT_RATIO_X1000: u32 = 1500;

pub fn color_image_from_slot(
    stream: &CaptureStreamDescriptor,
    frame: &VideoFrameDescriptor,
) -> Result<egui::ColorImage> {
    let width = frame.width.max(1) as usize;
    let height = frame.height.max(1) as usize;
    let byte_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .context("frame dimensions overflow RGBA byte length")?;
    let reader = FrameSlotReader::open(Path::new(&stream.slot_path), byte_len)?;
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [width, height],
        reader.read(),
    ))
}

#[derive(Debug, Default)]
pub struct PreviewInputBridge {
    armed: bool,
    last_pointer_pos: Option<Pos2>,
    device_pointer: Option<DevicePointerState>,
    buttons: u8,
    pointer_long_axis_units: Option<u32>,
}

impl PreviewInputBridge {
    pub fn is_armed(&self) -> bool {
        self.armed
    }

    pub fn reset(&mut self) {
        self.armed = false;
        self.last_pointer_pos = None;
        self.device_pointer = None;
        self.buttons = 0;
    }

    pub fn pointer_long_axis_units(&self) -> Option<u32> {
        self.pointer_long_axis_units
    }

    pub fn set_pointer_long_axis_units(&mut self, units: Option<u32>) {
        self.pointer_long_axis_units = units
            .map(|value| value.clamp(MIN_POINTER_LONG_AXIS_UNITS, MAX_POINTER_LONG_AXIS_UNITS));
        self.device_pointer = None;
    }

    pub fn release_control(&mut self) -> Vec<ControlInputEvent> {
        let mut events = Vec::new();
        self.release(&mut events);
        events
    }

    pub fn collect(
        &mut self,
        ctx: &egui::Context,
        response: &Response,
        frame_size: [u32; 2],
    ) -> Vec<ControlInputEvent> {
        let mut events = Vec::new();
        let rect = response.rect;

        let pointer_inside = ctx
            .input(|input| input.pointer.latest_pos())
            .is_some_and(|pos| rect.contains(pos));
        let viewport_focused = ctx.input(|input| input.viewport().focused.unwrap_or(true));
        if self.armed && (!pointer_inside || !viewport_focused) {
            self.release(&mut events);
            return events;
        }

        let input_events = ctx.input(|input| input.events.clone());
        for event in input_events {
            match event {
                Event::PointerMoved(pos) => {
                    self.handle_pointer_moved(pos, rect, frame_size, &mut events);
                }
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    ..
                } => {
                    self.handle_pointer_button(pos, button, pressed, rect, frame_size, &mut events);
                }
                Event::PointerGone => {
                    self.release(&mut events);
                }
                Event::MouseWheel { delta, .. } => {
                    if self.armed && pointer_inside {
                        let wheel = wheel_delta(delta.y);
                        if wheel != 0 {
                            events.push(ControlInputEvent::Mouse(MouseInputReport {
                                buttons: self.buttons,
                                dx: 0,
                                dy: 0,
                                wheel,
                            }));
                        }
                    }
                }
                Event::Key {
                    key,
                    pressed,
                    repeat: _,
                    modifiers,
                    ..
                } => {
                    if self.armed {
                        if is_host_paste_shortcut(key, modifiers) {
                            continue;
                        }
                        if let Some(report) = keyboard_report(key, pressed, modifiers) {
                            events.push(ControlInputEvent::Keyboard(report));
                        }
                    }
                }
                Event::Paste(text) => {
                    if self.armed && !text.is_empty() {
                        self.handle_paste_text(&text, &mut events);
                    }
                }
                Event::Text(text) => {
                    if self.armed && !text.is_empty() {
                        self.handle_text_event(&text, &mut events);
                    }
                }
                _ => {}
            }
        }

        events
    }

    fn handle_pointer_moved(
        &mut self,
        pos: Pos2,
        rect: Rect,
        frame_size: [u32; 2],
        events: &mut Vec<ControlInputEvent>,
    ) {
        let geometry = MirrorGeometry::new(rect, frame_size, self.pointer_long_axis_units);
        if !self.armed {
            if geometry.contains(pos) {
                self.last_pointer_pos = Some(pos);
            }
            return;
        }

        if !geometry.contains(pos) {
            self.release(events);
            return;
        }

        self.move_pointer_by_preview_delta(pos, geometry, events);
        self.last_pointer_pos = Some(pos);
    }

    fn handle_pointer_button(
        &mut self,
        pos: Pos2,
        button: PointerButton,
        pressed: bool,
        rect: Rect,
        frame_size: [u32; 2],
        events: &mut Vec<ControlInputEvent>,
    ) {
        let geometry = MirrorGeometry::new(rect, frame_size, self.pointer_long_axis_units);
        let inside = geometry.contains(pos);
        let starts_new_gesture = pressed && inside && self.buttons == 0;
        if pressed && inside {
            self.armed = true;
            self.last_pointer_pos = Some(pos);
        }
        if !self.armed {
            return;
        }

        if !inside {
            self.release(events);
            return;
        }

        let mut reports = Vec::new();
        if starts_new_gesture {
            self.queue_pointer_to_preview_position(pos, geometry, &mut reports);
        } else if !pressed && self.buttons != 0 {
            self.queue_pointer_by_preview_delta(pos, geometry, &mut reports);
        }

        let mask = button_mask(button);
        if pressed {
            self.buttons |= mask;
        } else {
            self.buttons &= !mask;
        }
        reports.push(MouseInputReport {
            buttons: self.buttons,
            dx: 0,
            dy: 0,
            wheel: 0,
        });
        push_mouse_reports(events, reports);
        self.last_pointer_pos = Some(pos);
    }

    fn release(&mut self, events: &mut Vec<ControlInputEvent>) {
        if self.armed || self.buttons != 0 {
            events.push(ControlInputEvent::Mouse(MouseInputReport::default()));
        }
        self.buttons = 0;
        self.armed = false;
        self.last_pointer_pos = None;
        self.device_pointer = None;
    }

    fn handle_paste_text(&self, text: &str, events: &mut Vec<ControlInputEvent>) {
        events.push(ControlInputEvent::Text(text.to_string()));
    }

    fn handle_text_event(&self, text: &str, events: &mut Vec<ControlInputEvent>) {
        if text == " " {
            events.push(ControlInputEvent::Text(text.to_string()));
        }
    }

    fn queue_pointer_to_preview_position(
        &mut self,
        pos: Pos2,
        geometry: MirrorGeometry,
        reports: &mut Vec<MouseInputReport>,
    ) {
        let target = geometry.pointer_position(pos);
        if self.device_pointer.map(|pointer| pointer.frame_size) != Some(geometry.frame_size) {
            reports.push(MouseInputReport {
                buttons: self.buttons,
                dx: -overshoot_delta(geometry.pointer_size[0]),
                dy: -overshoot_delta(geometry.pointer_size[1]),
                wheel: 0,
            });
            push_settle_reports(reports, self.buttons, POINTER_EDGE_SETTLE_REPORTS);
            self.device_pointer = Some(DevicePointerState {
                frame_size: geometry.frame_size,
                position: [0, 0],
            });
        }
        self.queue_device_pointer_to(target, geometry, reports);
        push_settle_reports(reports, self.buttons, POINTER_TARGET_SETTLE_REPORTS);
    }

    fn move_pointer_by_preview_delta(
        &mut self,
        pos: Pos2,
        geometry: MirrorGeometry,
        events: &mut Vec<ControlInputEvent>,
    ) {
        let mut reports = Vec::new();
        self.queue_pointer_by_preview_delta(pos, geometry, &mut reports);
        push_mouse_reports(events, reports);
    }

    fn queue_pointer_by_preview_delta(
        &mut self,
        pos: Pos2,
        geometry: MirrorGeometry,
        reports: &mut Vec<MouseInputReport>,
    ) {
        if self
            .device_pointer
            .is_some_and(|pointer| pointer.frame_size == geometry.frame_size)
        {
            let target = geometry.pointer_position(pos);
            self.queue_device_pointer_to(target, geometry, reports);
            return;
        }

        let Some(previous) = self.last_pointer_pos else {
            return;
        };
        let [dx, dy] = geometry.pointer_delta(previous, pos);
        if dx != 0 || dy != 0 {
            reports.push(MouseInputReport {
                buttons: self.buttons,
                dx,
                dy,
                wheel: 0,
            });
        }
    }

    fn queue_device_pointer_to(
        &mut self,
        target: [i32; 2],
        geometry: MirrorGeometry,
        reports: &mut Vec<MouseInputReport>,
    ) {
        if self.device_pointer.map(|pointer| pointer.frame_size) != Some(geometry.frame_size) {
            self.device_pointer = Some(DevicePointerState {
                frame_size: geometry.frame_size,
                position: target,
            });
            return;
        }

        let pointer = self
            .device_pointer
            .as_mut()
            .expect("device pointer initialized above");
        let dx = clamp_mouse_axis(target[0] - pointer.position[0]);
        let dy = clamp_mouse_axis(target[1] - pointer.position[1]);
        if dx != 0 || dy != 0 {
            reports.push(MouseInputReport {
                buttons: self.buttons,
                dx,
                dy,
                wheel: 0,
            });
        }
        pointer.position = target;
    }
}

#[derive(Debug, Clone, Copy)]
struct DevicePointerState {
    frame_size: [u32; 2],
    position: [i32; 2],
}

#[derive(Debug, Clone, Copy)]
struct MirrorGeometry {
    rect: Rect,
    frame_size: [u32; 2],
    pointer_size: [u32; 2],
}

impl MirrorGeometry {
    fn new(rect: Rect, frame_size: [u32; 2], pointer_long_axis_units: Option<u32>) -> Self {
        let frame_size = [frame_size[0].max(1), frame_size[1].max(1)];
        Self {
            rect,
            frame_size,
            pointer_size: pointer_size_for_frame(frame_size, pointer_long_axis_units),
        }
    }

    fn contains(&self, pos: Pos2) -> bool {
        self.rect.contains(pos)
    }

    fn pointer_delta(&self, previous: Pos2, current: Pos2) -> [i16; 2] {
        let delta = current - previous;
        [
            scaled_delta(delta.x, self.pointer_size[0], self.rect.width()),
            scaled_delta(delta.y, self.pointer_size[1], self.rect.height()),
        ]
    }

    fn pointer_position(&self, pos: Pos2) -> [i32; 2] {
        [
            scaled_absolute_axis(
                pos.x - self.rect.left(),
                self.pointer_size[0],
                self.rect.width(),
            ),
            scaled_absolute_axis(
                pos.y - self.rect.top(),
                self.pointer_size[1],
                self.rect.height(),
            ),
        ]
    }
}

fn pointer_size_for_frame(frame_size: [u32; 2], override_units: Option<u32>) -> [u32; 2] {
    let long_axis = pointer_long_axis_units_for_frame(frame_size, override_units);
    pointer_size_for_frame_with_long_axis(frame_size, long_axis)
}

fn pointer_size_for_frame_with_long_axis(frame_size: [u32; 2], long_axis: u32) -> [u32; 2] {
    let frame_long_axis = frame_size[0].max(frame_size[1]).max(1);
    [
        scaled_pointer_axis(frame_size[0], frame_long_axis, long_axis),
        scaled_pointer_axis(frame_size[1], frame_long_axis, long_axis),
    ]
}

fn pointer_long_axis_units_for_frame(frame_size: [u32; 2], override_units: Option<u32>) -> u32 {
    override_units
        .map(|value| value.clamp(MIN_POINTER_LONG_AXIS_UNITS, MAX_POINTER_LONG_AXIS_UNITS))
        .or_else(|| {
            std::env::var(POINTER_LONG_AXIS_UNITS_ENV)
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
                .map(|value| value.clamp(MIN_POINTER_LONG_AXIS_UNITS, MAX_POINTER_LONG_AXIS_UNITS))
        })
        .unwrap_or_else(|| default_pointer_long_axis_units_for_frame(frame_size))
}

pub fn pointer_long_axis_units_from_env() -> Option<u32> {
    std::env::var(POINTER_LONG_AXIS_UNITS_ENV)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .map(|value| value.clamp(MIN_POINTER_LONG_AXIS_UNITS, MAX_POINTER_LONG_AXIS_UNITS))
}

fn default_pointer_long_axis_units_for_frame(frame_size: [u32; 2]) -> u32 {
    let short_axis = frame_size[0].min(frame_size[1]).max(1);
    let long_axis = frame_size[0].max(frame_size[1]).max(1);
    let aspect_x1000 = (u64::from(long_axis) * 1000 / u64::from(short_axis)) as u32;
    if aspect_x1000 <= TABLET_ASPECT_RATIO_X1000 {
        DEFAULT_TABLET_POINTER_LONG_AXIS_UNITS
    } else {
        DEFAULT_PHONE_POINTER_LONG_AXIS_UNITS
    }
}

fn scaled_pointer_axis(frame_axis: u32, frame_long_axis: u32, pointer_long_axis: u32) -> u32 {
    ((u64::from(frame_axis) * u64::from(pointer_long_axis) + u64::from(frame_long_axis / 2))
        / u64::from(frame_long_axis))
    .clamp(1, u64::from(i16::MAX as u16)) as u32
}

fn push_mouse_reports(events: &mut Vec<ControlInputEvent>, reports: Vec<MouseInputReport>) {
    match reports.len() {
        0 => {}
        1 => events.push(ControlInputEvent::Mouse(reports[0])),
        _ => events.push(ControlInputEvent::MouseSequence(reports)),
    }
}

fn push_settle_reports(reports: &mut Vec<MouseInputReport>, buttons: u8, count: usize) {
    reports.extend((0..count).map(|_| MouseInputReport {
        buttons,
        dx: 0,
        dy: 0,
        wheel: 0,
    }));
}

fn scaled_delta(delta: f32, frame_axis: u32, preview_axis: f32) -> i16 {
    if preview_axis <= 0.0 {
        return 0;
    }
    (delta * frame_axis as f32 / preview_axis)
        .round()
        .clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

fn scaled_absolute_axis(value: f32, frame_axis: u32, preview_axis: f32) -> i32 {
    if preview_axis <= 0.0 {
        return 0;
    }
    let value = value.clamp(0.0, preview_axis);
    (value * frame_axis as f32 / preview_axis)
        .round()
        .clamp(0.0, i32::MAX as f32) as i32
}

fn overshoot_delta(frame_axis: u32) -> i16 {
    frame_axis.saturating_mul(2).min(i16::MAX as u32) as i16
}

fn clamp_mouse_axis(value: i32) -> i16 {
    value.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

fn button_mask(button: PointerButton) -> u8 {
    match button {
        PointerButton::Primary => 0x01,
        PointerButton::Secondary => 0x02,
        PointerButton::Middle => 0x04,
        _ => 0,
    }
}

fn wheel_delta(delta_y: f32) -> i8 {
    if delta_y > 0.0 {
        1
    } else if delta_y < 0.0 {
        -1
    } else {
        0
    }
}

fn keyboard_report(key: Key, pressed: bool, modifiers: Modifiers) -> Option<KeyboardInputReport> {
    let (usage_id, force_shift) = key_usage(key)?;
    let mut mapped_modifiers = key_modifiers(modifiers);
    mapped_modifiers.shift |= force_shift;
    Some(KeyboardInputReport {
        usage_id,
        modifiers: mapped_modifiers,
        pressed,
    })
}

fn is_host_paste_shortcut(key: Key, modifiers: Modifiers) -> bool {
    key == Key::V && (modifiers.command || modifiers.ctrl || modifiers.mac_cmd)
}

fn key_modifiers(modifiers: Modifiers) -> KeyModifiers {
    KeyModifiers {
        shift: modifiers.shift,
        alt: modifiers.alt,
        ctrl: false,
        meta: modifiers.command || modifiers.ctrl || modifiers.mac_cmd,
    }
}

fn key_usage(key: Key) -> Option<(u8, bool)> {
    let usage = match key {
        Key::A => (0x04, false),
        Key::B => (0x05, false),
        Key::C => (0x06, false),
        Key::D => (0x07, false),
        Key::E => (0x08, false),
        Key::F => (0x09, false),
        Key::G => (0x0a, false),
        Key::H => (0x0b, false),
        Key::I => (0x0c, false),
        Key::J => (0x0d, false),
        Key::K => (0x0e, false),
        Key::L => (0x0f, false),
        Key::M => (0x10, false),
        Key::N => (0x11, false),
        Key::O => (0x12, false),
        Key::P => (0x13, false),
        Key::Q => (0x14, false),
        Key::R => (0x15, false),
        Key::S => (0x16, false),
        Key::T => (0x17, false),
        Key::U => (0x18, false),
        Key::V => (0x19, false),
        Key::W => (0x1a, false),
        Key::X => (0x1b, false),
        Key::Y => (0x1c, false),
        Key::Z => (0x1d, false),
        Key::Num1 => (0x1e, false),
        Key::Num2 => (0x1f, false),
        Key::Num3 => (0x20, false),
        Key::Num4 => (0x21, false),
        Key::Num5 => (0x22, false),
        Key::Num6 => (0x23, false),
        Key::Num7 => (0x24, false),
        Key::Num8 => (0x25, false),
        Key::Num9 => (0x26, false),
        Key::Num0 => (0x27, false),
        Key::Enter => (0x28, false),
        Key::Escape => (0x29, false),
        Key::Backspace => (0x2a, false),
        Key::Tab => (0x2b, false),
        Key::Space => (0x2c, false),
        Key::Minus => (0x2d, false),
        Key::Equals => (0x2e, false),
        Key::OpenBracket => (0x2f, false),
        Key::CloseBracket => (0x30, false),
        Key::Backslash => (0x31, false),
        Key::Semicolon => (0x33, false),
        Key::Quote => (0x34, false),
        Key::Backtick => (0x35, false),
        Key::Comma => (0x36, false),
        Key::Period => (0x37, false),
        Key::Slash => (0x38, false),
        Key::Delete => (0x4c, false),
        Key::ArrowRight => (0x4f, false),
        Key::ArrowLeft => (0x50, false),
        Key::ArrowDown => (0x51, false),
        Key::ArrowUp => (0x52, false),
        Key::Exclamationmark => (0x1e, true),
        Key::Questionmark => (0x38, true),
        Key::Colon => (0x33, true),
        Key::Plus => (0x2e, true),
        Key::Pipe => (0x31, true),
        Key::OpenCurlyBracket => (0x2f, true),
        Key::CloseCurlyBracket => (0x30, true),
        _ => return None,
    };
    Some(usage)
}

#[cfg(test)]
mod input_tests {
    use super::*;

    #[test]
    fn key_mapper_maps_windows_command_to_ios_meta() {
        let report = keyboard_report(
            Key::L,
            true,
            Modifiers {
                ctrl: true,
                command: true,
                ..Modifiers::NONE
            },
        )
        .expect("key should map");

        assert_eq!(report.usage_id, 0x0f);
        assert!(report.modifiers.meta);
        assert!(!report.modifiers.ctrl);
    }

    #[test]
    fn key_mapper_maps_windows_ctrl_only_to_ios_meta() {
        let report = keyboard_report(
            Key::L,
            true,
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        )
        .expect("key should map");

        assert_eq!(report.usage_id, 0x0f);
        assert!(report.modifiers.meta);
        assert!(!report.modifiers.ctrl);
    }

    #[test]
    fn shifted_punctuation_forces_shift_modifier() {
        let report =
            keyboard_report(Key::Questionmark, true, Modifiers::NONE).expect("key should map");

        assert_eq!(report.usage_id, 0x38);
        assert!(report.modifiers.shift);
    }

    #[test]
    fn paste_text_emits_single_batched_text_event() {
        let bridge = PreviewInputBridge::default();
        let mut events = Vec::new();

        bridge.handle_paste_text("Az9!", &mut events);

        assert_eq!(events, vec![ControlInputEvent::Text("Az9!".into())]);
    }

    #[test]
    fn text_event_space_forwards_for_ime_candidate_selection() {
        let bridge = PreviewInputBridge::default();
        let mut events = Vec::new();

        bridge.handle_text_event(" ", &mut events);
        bridge.handle_text_event("a", &mut events);

        assert_eq!(events, vec![ControlInputEvent::Text(" ".into())]);
    }

    #[test]
    fn windows_paste_shortcut_is_intercepted_by_host() {
        assert!(is_host_paste_shortcut(
            Key::V,
            Modifiers {
                ctrl: true,
                command: true,
                ..Modifiers::NONE
            }
        ));
        assert!(!is_host_paste_shortcut(Key::C, Modifiers::COMMAND));
    }

    #[test]
    fn preview_delta_scales_to_frame_space() {
        assert_eq!(scaled_delta(10.0, 1000, 250.0), 40);
    }

    #[test]
    fn pointer_size_uses_ios_logical_scale_not_video_pixels() {
        assert_eq!(
            default_pointer_long_axis_units_for_frame([1080, 1920]),
            DEFAULT_PHONE_POINTER_LONG_AXIS_UNITS
        );
        assert_eq!(
            default_pointer_long_axis_units_for_frame([2048, 2732]),
            DEFAULT_TABLET_POINTER_LONG_AXIS_UNITS
        );
        assert_eq!(
            pointer_size_for_frame_with_long_axis(
                [1080, 1920],
                DEFAULT_PHONE_POINTER_LONG_AXIS_UNITS,
            ),
            [68, 120]
        );
    }

    #[test]
    fn pointer_size_can_use_live_bridge_override() {
        let mut bridge = PreviewInputBridge::default();

        bridge.set_pointer_long_axis_units(Some(60));

        assert_eq!(bridge.pointer_long_axis_units(), Some(60));
        assert_eq!(
            pointer_size_for_frame([1000, 2000], bridge.pointer_long_axis_units()),
            [30, 60]
        );
    }

    #[test]
    fn pointer_size_override_is_clamped() {
        let mut bridge = PreviewInputBridge::default();

        bridge.set_pointer_long_axis_units(Some(1));

        assert_eq!(
            bridge.pointer_long_axis_units(),
            Some(MIN_POINTER_LONG_AXIS_UNITS)
        );
    }

    #[test]
    fn preview_click_inside_arms_ble_mouse_forwarding() {
        let mut bridge = PreviewInputBridge::default();
        let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), egui::vec2(100.0, 200.0));
        let mut events = Vec::new();

        bridge.handle_pointer_moved(Pos2::new(20.0, 20.0), rect, [1000, 2000], &mut events);
        assert!(!bridge.is_armed());
        assert!(events.is_empty());

        bridge.handle_pointer_button(
            Pos2::new(20.0, 20.0),
            PointerButton::Primary,
            true,
            rect,
            [1000, 2000],
            &mut events,
        );

        assert!(bridge.is_armed());
        assert_eq!(
            events,
            vec![ControlInputEvent::MouseSequence(vec![
                mouse(0, -120, -240),
                mouse(0, 0, 0),
                mouse(0, 0, 0),
                mouse(0, 0, 0),
                mouse(0, 6, 6),
                mouse(0, 0, 0),
                mouse(0, 0, 0),
                mouse(0, 0, 0),
                mouse(0x01, 0, 0),
            ])]
        );
    }

    #[test]
    fn preview_mouse_move_inside_forwards_scaled_ble_delta() {
        let mut bridge = PreviewInputBridge::default();
        let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), egui::vec2(100.0, 200.0));
        let mut events = Vec::new();

        bridge.handle_pointer_button(
            Pos2::new(20.0, 20.0),
            PointerButton::Primary,
            true,
            rect,
            [1000, 2000],
            &mut events,
        );
        events.clear();

        bridge.handle_pointer_moved(Pos2::new(25.0, 30.0), rect, [1000, 2000], &mut events);

        assert_eq!(
            events,
            vec![ControlInputEvent::Mouse(MouseInputReport {
                buttons: 0x01,
                dx: 3,
                dy: 6,
                wheel: 0,
            })]
        );
    }

    #[test]
    fn preview_repositions_fresh_clicks_from_tracked_device_pointer() {
        let mut bridge = PreviewInputBridge::default();
        let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), egui::vec2(100.0, 200.0));
        let mut events = Vec::new();

        bridge.handle_pointer_button(
            Pos2::new(20.0, 20.0),
            PointerButton::Primary,
            true,
            rect,
            [1000, 2000],
            &mut events,
        );
        bridge.handle_pointer_button(
            Pos2::new(20.0, 20.0),
            PointerButton::Primary,
            false,
            rect,
            [1000, 2000],
            &mut events,
        );
        events.clear();

        bridge.handle_pointer_button(
            Pos2::new(60.0, 110.0),
            PointerButton::Primary,
            true,
            rect,
            [1000, 2000],
            &mut events,
        );

        assert_eq!(
            events,
            vec![ControlInputEvent::MouseSequence(vec![
                mouse(0, 24, 54),
                mouse(0, 0, 0),
                mouse(0, 0, 0),
                mouse(0, 0, 0),
                mouse(0x01, 0, 0),
            ])]
        );
    }

    #[test]
    fn preview_release_uses_release_position_for_drag_without_move_event() {
        let mut bridge = PreviewInputBridge::default();
        let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), egui::vec2(100.0, 200.0));
        let mut events = Vec::new();

        bridge.handle_pointer_button(
            Pos2::new(20.0, 20.0),
            PointerButton::Primary,
            true,
            rect,
            [1000, 2000],
            &mut events,
        );
        events.clear();

        bridge.handle_pointer_button(
            Pos2::new(30.0, 50.0),
            PointerButton::Primary,
            false,
            rect,
            [1000, 2000],
            &mut events,
        );

        assert_eq!(
            events,
            vec![ControlInputEvent::MouseSequence(vec![
                mouse(0x01, 6, 18),
                mouse(0, 0, 0),
            ])]
        );
    }

    #[test]
    fn preview_pointer_exit_releases_ble_mouse_control() {
        let mut bridge = PreviewInputBridge::default();
        let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), egui::vec2(100.0, 200.0));
        let mut events = Vec::new();

        bridge.handle_pointer_button(
            Pos2::new(20.0, 20.0),
            PointerButton::Primary,
            true,
            rect,
            [1000, 2000],
            &mut events,
        );
        events.clear();

        bridge.handle_pointer_moved(Pos2::new(120.0, 30.0), rect, [1000, 2000], &mut events);

        assert!(!bridge.is_armed());
        assert_eq!(
            events,
            vec![ControlInputEvent::Mouse(MouseInputReport::default())]
        );
    }

    #[test]
    fn preview_pointer_exit_releases_ble_control_after_click_is_released() {
        let mut bridge = PreviewInputBridge::default();
        let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), egui::vec2(100.0, 200.0));
        let mut events = Vec::new();

        bridge.handle_pointer_button(
            Pos2::new(20.0, 20.0),
            PointerButton::Primary,
            true,
            rect,
            [1000, 2000],
            &mut events,
        );
        bridge.handle_pointer_button(
            Pos2::new(20.0, 20.0),
            PointerButton::Primary,
            false,
            rect,
            [1000, 2000],
            &mut events,
        );
        events.clear();

        bridge.handle_pointer_moved(Pos2::new(120.0, 30.0), rect, [1000, 2000], &mut events);

        assert!(!bridge.is_armed());
        assert_eq!(
            events,
            vec![ControlInputEvent::Mouse(MouseInputReport::default())]
        );
    }

    fn mouse(buttons: u8, dx: i16, dy: i16) -> MouseInputReport {
        MouseInputReport {
            buttons,
            dx,
            dy,
            wheel: 0,
        }
    }
}
