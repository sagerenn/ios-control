use egui::Ui;

use crate::preview::{
    DEFAULT_PHONE_POINTER_LONG_AXIS_UNITS, MAX_POINTER_LONG_AXIS_UNITS, MIN_POINTER_LONG_AXIS_UNITS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsAction {
    None,
    SetBlePointerLongAxisUnits(Option<u32>),
}

pub fn render_rows(
    ui: &mut Ui,
    rows: &[String],
    ble_pointer_long_axis_units: Option<u32>,
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

    action
}
