use egui::Ui;

use crate::preferences::{
    DEFAULT_DIRECT_PREVIEW_FPS, DEFAULT_DIRECT_PREVIEW_HEIGHT, MAX_DIRECT_PREVIEW_FPS,
    MAX_DIRECT_PREVIEW_HEIGHT, MIN_DIRECT_PREVIEW_FPS, MIN_DIRECT_PREVIEW_HEIGHT,
};
use crate::preview::{
    DEFAULT_PHONE_POINTER_LONG_AXIS_UNITS, MAX_POINTER_LONG_AXIS_UNITS, MIN_POINTER_LONG_AXIS_UNITS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsAction {
    None,
    SetBlePointerLongAxisUnits(Option<u32>),
    SetDirectPreviewFps(Option<u32>),
    SetDirectPreviewHeight(Option<u32>),
}

pub fn render_rows(
    ui: &mut Ui,
    rows: &[String],
    ble_pointer_long_axis_units: Option<u32>,
    direct_preview_fps: Option<u32>,
    direct_preview_height: Option<u32>,
) -> SettingsAction {
    let mut action = SettingsAction::None;

    ui.heading("Settings");
    for row in rows {
        ui.label(row);
    }

    ui.horizontal(|ui| {
        ui.label("BLE pointer scale");
        let mut auto = ble_pointer_long_axis_units.is_none();
        let mut units =
            ble_pointer_long_axis_units.unwrap_or(DEFAULT_PHONE_POINTER_LONG_AXIS_UNITS);
        if ui.checkbox(&mut auto, "Auto").changed() {
            action = if auto {
                SettingsAction::SetBlePointerLongAxisUnits(None)
            } else {
                SettingsAction::SetBlePointerLongAxisUnits(Some(units))
            };
        }

        let response = ui.add_enabled(
            !auto,
            egui::DragValue::new(&mut units)
                .range(MIN_POINTER_LONG_AXIS_UNITS..=MAX_POINTER_LONG_AXIS_UNITS)
                .speed(1)
                .suffix(" units"),
        );
        if response.changed() {
            action = SettingsAction::SetBlePointerLongAxisUnits(Some(units));
        }

        if ui.button("Reset").clicked() {
            action = SettingsAction::SetBlePointerLongAxisUnits(None);
        }
    });

    ui.horizontal(|ui| {
        ui.label("Direct preview FPS");
        let mut fps = direct_preview_fps.unwrap_or(DEFAULT_DIRECT_PREVIEW_FPS);
        let response = ui.add(
            egui::DragValue::new(&mut fps)
                .range(MIN_DIRECT_PREVIEW_FPS..=MAX_DIRECT_PREVIEW_FPS)
                .speed(1)
                .suffix(" fps"),
        );
        if response.changed() {
            action = SettingsAction::SetDirectPreviewFps(Some(fps));
        }

        if ui.button("Reset").clicked() {
            action = SettingsAction::SetDirectPreviewFps(None);
        }
    });

    ui.horizontal(|ui| {
        ui.label("Direct preview height");
        let mut height = direct_preview_height.unwrap_or(DEFAULT_DIRECT_PREVIEW_HEIGHT);
        let response = ui.add(
            egui::DragValue::new(&mut height)
                .range(MIN_DIRECT_PREVIEW_HEIGHT..=MAX_DIRECT_PREVIEW_HEIGHT)
                .speed(8)
                .suffix(" px"),
        );
        if response.changed() {
            action = SettingsAction::SetDirectPreviewHeight(Some(height));
        }

        if ui.button("Reset").clicked() {
            action = SettingsAction::SetDirectPreviewHeight(None);
        }
    });

    action
}
