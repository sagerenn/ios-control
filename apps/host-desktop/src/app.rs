use eframe::egui;

use crate::panels::dashboard;
use crate::view_models::dashboard::DashboardViewModel;

pub struct HostDesktopApp {
    pub dashboard: DashboardViewModel,
}

impl eframe::App for HostDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| dashboard::render(ui, &self.dashboard));
    }
}
