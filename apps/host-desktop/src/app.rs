use eframe::egui;

use crate::panels::device_detail::{CaptureSourceOption, ControlSetupChecklist};
use crate::panels::{dashboard, device_detail, diagnostics, session_view, settings};
use crate::view_models::dashboard::DashboardViewModel;
use crate::view_models::device_detail::DeviceDetailViewModel;
use crate::view_models::diagnostics::DiagnosticsViewModel;
use crate::view_models::session::SessionViewModel;
use crate::view_models::settings::SettingsViewModel;

pub struct HostDesktopApp {
    pub dashboard: DashboardViewModel,
    pub device_detail: DeviceDetailViewModel,
    pub session: SessionViewModel,
    pub diagnostics: DiagnosticsViewModel,
    pub settings: SettingsViewModel,
}

impl HostDesktopApp {
    pub fn demo() -> Self {
        Self {
            dashboard: DashboardViewModel {
                total_devices: 1,
                degraded_devices: 0,
            },
            device_detail: DeviceDetailViewModel {
                device_name: "Mock iPhone".into(),
                capture_sources: vec![CaptureSourceOption::new(
                    "window:mock",
                    "Mock iPhone Mirror",
                )],
                control_checklist: ControlSetupChecklist::for_pointer_mode(),
            },
            session: SessionViewModel {
                selected_source: None,
                latest_frame: None,
            },
            diagnostics: DiagnosticsViewModel {
                control_summary: "control not started".into(),
                grounding_summary: "grounding idle".into(),
            },
            settings: SettingsViewModel {
                plugin_rows: vec![
                    "capture.window".into(),
                    "control.ble".into(),
                    "grounding.core".into(),
                ],
            },
        }
    }
}

impl eframe::App for HostDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            dashboard::render(ui, &self.dashboard);
            ui.separator();
            device_detail::render(
                ui,
                &self.device_detail.device_name,
                &self.device_detail.capture_sources,
                &self.device_detail.control_checklist,
            );
            ui.separator();
            session_view::render(ui, &self.session);
            ui.separator();
            diagnostics::render(ui, &self.diagnostics.grounding_summary);
            diagnostics::render_control_diagnostics(ui, &self.diagnostics.control_summary);
            ui.separator();
            settings::render_rows(ui, &self.settings.plugin_rows);
        });
    }
}
