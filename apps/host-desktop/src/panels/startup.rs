use egui::Ui;

use crate::view_models::startup::StartupViewModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupAction {
    None,
    StartDirectReceiver,
}

pub fn render(ui: &mut Ui, view_model: &StartupViewModel) -> StartupAction {
    let mut action = StartupAction::None;

    ui.heading("Startup Readiness");
    ui.label(&view_model.summary);
    ui.horizontal(|ui| {
        ui.label(format!(
            "Direct Receiver | {} | {}",
            view_model.direct_receiver.status, view_model.direct_receiver.detail
        ));
        if ui
            .add_enabled(
                view_model.direct_receiver.available,
                egui::Button::new("Start Direct Receiver"),
            )
            .clicked()
        {
            action = StartupAction::StartDirectReceiver;
        }
    });
    for item in &view_model.items {
        ui.label(format!(
            "{} | {} | {}",
            item.label, item.status, item.detail
        ));
    }

    action
}
