use eframe::egui;
use ios_control_contracts::capture::{FrameHealth, SourceKind, VideoFrameDescriptor};

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
        }
    }

    pub fn demo() -> Self {
        Self::new()
    }

    pub fn request_start_session(&mut self) {
        self.session = SessionViewModel::starting();
        self.diagnostics.host_error = None;
        self.diagnostics.control_summary = "control bootstrapping".into();
        self.diagnostics.grounding_summary = "grounding bootstrapping".into();
    }

    pub fn finish_pending_session_start(&mut self) {
        let Some(source) = self.device_detail.capture_sources.first().cloned() else {
            let message = "No capture sources available";
            self.session = SessionViewModel::error(message);
            self.device_detail.active_source_id = None;
            self.diagnostics.host_error = Some(message.into());
            self.diagnostics.control_summary = "control blocked".into();
            self.diagnostics.grounding_summary = "grounding blocked".into();
            return;
        };

        let frame = VideoFrameDescriptor {
            source_id: source.source_id.clone(),
            source_kind: if source.source_id.starts_with("window") {
                SourceKind::Window
            } else {
                SourceKind::DirectReceiver
            },
            width: 1280,
            height: 720,
            rotation_degrees: 0,
            frame_index: 1,
            health: FrameHealth::Healthy,
        };

        self.device_detail.active_source_id = Some(source.source_id.clone());
        self.session = SessionViewModel::streaming(source, frame);
        self.diagnostics.host_error = None;
        self.diagnostics.control_summary = "Connected to mock control session".into();
        self.diagnostics.grounding_summary = "grounding ready".into();
    }

    pub fn stop_session(&mut self) {
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
            session_view::render(ui, &self.session);
            ui.separator();
            diagnostics::render(ui, &self.diagnostics.grounding_summary);
            diagnostics::render_control_diagnostics(ui, &self.diagnostics.control_summary);
            ui.separator();
            settings::render_rows(ui, &self.settings.plugin_rows);
        });
    }
}
