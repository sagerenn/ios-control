use eframe::egui;

use crate::panels::device_detail::{CaptureSourceOption, ControlSetupChecklist};
use crate::panels::{dashboard, device_detail, diagnostics, session_view, settings};
use crate::panels::session_view::SessionAction;
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
    pending_session_start: Option<u8>,
}

impl HostDesktopApp {
    pub fn new() -> Self {
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
                active_source_id: None,
                control_checklist: ControlSetupChecklist::for_pointer_mode(),
            },
            session: SessionViewModel::idle(),
            diagnostics: DiagnosticsViewModel {
                host_error: None,
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
            pending_session_start: None,
        }
    }

    pub fn demo() -> Self {
        Self::new()
    }

    pub fn request_start_session(&mut self) {
        self.session = SessionViewModel::starting();
        self.device_detail.active_source_id = None;
        self.diagnostics.host_error = None;
        self.diagnostics.control_summary = "control bootstrapping".into();
        self.diagnostics.grounding_summary = "grounding bootstrapping".into();
        self.pending_session_start = Some(1);
    }

    pub fn finish_pending_session_start(&mut self) {
        self.pending_session_start = None;
        let Some(source) = self.device_detail.capture_sources.first().cloned() else {
            let message = "No capture sources available";
            self.session = SessionViewModel::error(message);
            self.device_detail.active_source_id = None;
            self.diagnostics.host_error = Some(message.into());
            self.diagnostics.control_summary = "control blocked".into();
            self.diagnostics.grounding_summary = "grounding blocked".into();
            return;
        };

        let _ = source;
        let message = "Session bootstrap is not wired to the runtime yet";
        self.device_detail.active_source_id = None;
        self.session = SessionViewModel::error(message);
        self.diagnostics.host_error = Some(message.into());
        self.diagnostics.control_summary = "control blocked".into();
        self.diagnostics.grounding_summary = "grounding blocked".into();
    }

    pub fn stop_session(&mut self) {
        self.pending_session_start = None;
        self.device_detail.active_source_id = None;
        self.session = SessionViewModel::idle();
        self.diagnostics.host_error = None;
        self.diagnostics.control_summary = "control not started".into();
        self.diagnostics.grounding_summary = "grounding idle".into();
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
            match session_view::render(ui, &self.session) {
                SessionAction::None => {}
                SessionAction::Start => {
                    self.request_start_session();
                    ctx.request_repaint();
                }
                SessionAction::Stop => {
                    self.stop_session();
                }
            }
            ui.separator();
            let diagnostic_message = match &self.diagnostics.host_error {
                Some(error) => format!("{} | {}", self.diagnostics.grounding_summary, error),
                None => self.diagnostics.grounding_summary.clone(),
            };
            diagnostics::render(ui, &diagnostic_message);
            diagnostics::render_control_diagnostics(ui, &self.diagnostics.control_summary);
            ui.separator();
            settings::render_rows(ui, &self.settings.plugin_rows);
        });

        match self.pending_session_start {
            Some(0) => {
                self.finish_pending_session_start();
                ctx.request_repaint();
            }
            Some(steps) => {
                self.pending_session_start = Some(steps - 1);
                ctx.request_repaint();
            }
            None => {}
        }
    }
}
