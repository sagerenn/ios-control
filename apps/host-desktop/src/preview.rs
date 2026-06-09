use std::path::Path;

use anyhow::{Context, Result};
use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, Response};
use ios_control_contracts::capture::{CaptureStreamDescriptor, VideoFrameDescriptor};
use ios_control_contracts::control::{
    ControlInputEvent, KeyModifiers, KeyboardInputReport, MouseInputReport,
};
use ios_control_frame_transport::FrameSlotReader;

const POINTER_WAKE_DELTA: i16 = 1;

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
    buttons: u8,
}

impl PreviewInputBridge {
    pub fn is_armed(&self) -> bool {
        self.armed
    }

    pub fn reset(&mut self) {
        self.armed = false;
        self.last_pointer_pos = None;
        self.buttons = 0;
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
                    self.handle_pointer_button(pos, button, pressed, rect, &mut events);
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
        if !self.armed {
            if rect.contains(pos) {
                self.last_pointer_pos = Some(pos);
            }
            return;
        }

        if !rect.contains(pos) {
            self.release(events);
            return;
        }

        if let Some(previous) = self.last_pointer_pos {
            let delta = pos - previous;
            let dx = scaled_delta(delta.x, frame_size[0], rect.width());
            let dy = scaled_delta(delta.y, frame_size[1], rect.height());
            if dx != 0 || dy != 0 {
                events.push(ControlInputEvent::Mouse(MouseInputReport {
                    buttons: self.buttons,
                    dx,
                    dy,
                    wheel: 0,
                }));
            }
        }
        self.last_pointer_pos = Some(pos);
    }

    fn handle_pointer_button(
        &mut self,
        pos: Pos2,
        button: PointerButton,
        pressed: bool,
        rect: Rect,
        events: &mut Vec<ControlInputEvent>,
    ) {
        let inside = rect.contains(pos);
        let newly_armed = pressed && inside && !self.armed;
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

        if newly_armed {
            events.push(ControlInputEvent::Mouse(MouseInputReport {
                buttons: self.buttons,
                dx: POINTER_WAKE_DELTA,
                dy: 0,
                wheel: 0,
            }));
            events.push(ControlInputEvent::Mouse(MouseInputReport {
                buttons: self.buttons,
                dx: -POINTER_WAKE_DELTA,
                dy: 0,
                wheel: 0,
            }));
        }

        let mask = button_mask(button);
        if pressed {
            self.buttons |= mask;
        } else {
            self.buttons &= !mask;
        }
        events.push(ControlInputEvent::Mouse(MouseInputReport {
            buttons: self.buttons,
            dx: 0,
            dy: 0,
            wheel: 0,
        }));
    }

    fn release(&mut self, events: &mut Vec<ControlInputEvent>) {
        if self.armed || self.buttons != 0 {
            events.push(ControlInputEvent::Mouse(MouseInputReport::default()));
        }
        self.buttons = 0;
        self.armed = false;
        self.last_pointer_pos = None;
    }

    fn handle_paste_text(&self, text: &str, events: &mut Vec<ControlInputEvent>) {
        events.push(ControlInputEvent::Text(text.to_string()));
    }
}

fn scaled_delta(delta: f32, frame_axis: u32, preview_axis: f32) -> i16 {
    if preview_axis <= 0.0 {
        return 0;
    }
    (delta * frame_axis as f32 / preview_axis)
        .round()
        .clamp(i16::MIN as f32, i16::MAX as f32) as i16
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
        meta: modifiers.command || modifiers.mac_cmd,
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
            &mut events,
        );

        assert!(bridge.is_armed());
        assert_eq!(
            events,
            vec![
                ControlInputEvent::Mouse(MouseInputReport {
                    buttons: 0,
                    dx: POINTER_WAKE_DELTA,
                    dy: 0,
                    wheel: 0,
                }),
                ControlInputEvent::Mouse(MouseInputReport {
                    buttons: 0,
                    dx: -POINTER_WAKE_DELTA,
                    dy: 0,
                    wheel: 0,
                }),
                ControlInputEvent::Mouse(MouseInputReport {
                    buttons: 0x01,
                    dx: 0,
                    dy: 0,
                    wheel: 0,
                })
            ]
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
            &mut events,
        );
        events.clear();

        bridge.handle_pointer_moved(Pos2::new(25.0, 30.0), rect, [1000, 2000], &mut events);

        assert_eq!(
            events,
            vec![ControlInputEvent::Mouse(MouseInputReport {
                buttons: 0x01,
                dx: 50,
                dy: 100,
                wheel: 0,
            })]
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
            &mut events,
        );
        bridge.handle_pointer_button(
            Pos2::new(20.0, 20.0),
            PointerButton::Primary,
            false,
            rect,
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
}
