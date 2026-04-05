use eframe::egui;
use ios_control_contracts::session::{DeviceSessionStatus, SessionSubstate};

use crate::panels::device_detail::{
    CaptureSourceOption, ControlSetupChecklist, DeviceDetailAction,
};
use crate::panels::session_view::SessionAction;
use crate::panels::{dashboard, device_detail, diagnostics, session_view, settings};
use crate::preview::color_image_from_slot;
use crate::runtime::{HostRuntime, HostRuntimeConfig, HostRuntimeSnapshot, RuntimeWorkspaceState};
use crate::view_models::dashboard::DashboardViewModel;
use crate::view_models::device_detail::DeviceDetailViewModel;
use crate::view_models::diagnostics::DiagnosticsViewModel;
use crate::view_models::fleet::FleetViewModel;
use crate::view_models::session::{SessionUiState, SessionViewModel};
use crate::view_models::settings::SettingsViewModel;

pub struct HostDesktopApp {
    pub available_device_ids: Vec<String>,
    pub selected_device_id: Option<String>,
    pub fleet: FleetViewModel,
    runtime_statuses: Vec<DeviceSessionStatus>,
    host_runtime: Option<HostRuntime>,
    runtime_workspace: Option<RuntimeWorkspaceState>,
    preview_texture: Option<egui::TextureHandle>,
    pub dashboard: DashboardViewModel,
    pub device_detail: DeviceDetailViewModel,
    pub session: SessionViewModel,
    pub diagnostics: DiagnosticsViewModel,
    pub settings: SettingsViewModel,
}

impl HostDesktopApp {
    pub fn new() -> Self {
        Self {
            available_device_ids: Vec::new(),
            selected_device_id: None,
            fleet: FleetViewModel { rows: Vec::new() },
            runtime_statuses: Vec::new(),
            host_runtime: None,
            runtime_workspace: None,
            preview_texture: None,
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

    pub fn with_runtime(config: HostRuntimeConfig) -> Self {
        let mut app = Self::new();
        app.host_runtime =
            Some(HostRuntime::new(config).expect("host runtime should initialize successfully"));
        app
    }

    pub fn replace_runtime_statuses(&mut self, statuses: Vec<DeviceSessionStatus>) {
        self.runtime_workspace = None;
        self.runtime_statuses = statuses;
        self.sync_from_runtime();
    }

    pub fn select_device(&mut self, device_id: &str) {
        self.selected_device_id = Some(device_id.into());
        self.sync_selected_workspace();
    }

    pub fn start_runtime_session_on_launch(&mut self) {
        if self.selected_device_id.is_some() {
            self.request_start_session();
        }
    }

    pub fn request_start_session(&mut self) {
        let Some(host_runtime) = self.host_runtime.as_mut() else {
            let message = "Host runtime unavailable";
            self.session = SessionViewModel::error(message);
            self.diagnostics.host_error = Some(message.into());
            self.diagnostics.control_summary = "control blocked".into();
            self.diagnostics.grounding_summary = "grounding blocked".into();
            return;
        };

        let Some(device_id) = self
            .selected_device_id
            .clone()
            .or_else(|| self.available_device_ids.first().cloned())
        else {
            self.session = SessionViewModel::error("No device selected");
            return;
        };

        let selected_source_id = self.device_detail.active_source_id.clone();
        self.selected_device_id = Some(device_id.clone());
        self.session = SessionViewModel::starting();
        self.diagnostics.host_error = None;
        self.diagnostics.control_summary = "control bootstrapping".into();
        self.diagnostics.grounding_summary = "grounding bootstrapping".into();

        match host_runtime.start_session(
            &device_id,
            &self.device_detail.device_name,
            selected_source_id,
        ) {
            Ok(snapshot) => self.apply_runtime_snapshot(snapshot),
            Err(error) => {
                let message = error.to_string();
                self.session = SessionViewModel::error(&message);
                self.diagnostics.host_error = Some(message);
                self.diagnostics.control_summary = "control blocked".into();
                self.diagnostics.grounding_summary = "grounding blocked".into();
            }
        }
    }

    pub fn stop_session(&mut self) {
        if let (Some(host_runtime), Some(device_id)) = (
            self.host_runtime.as_mut(),
            self.selected_device_id.as_deref(),
        ) {
            let _ = host_runtime.stop_session(device_id);
        }

        self.runtime_statuses.clear();
        self.runtime_workspace = None;
        self.preview_texture = None;
        self.available_device_ids.clear();
        self.selected_device_id = None;
        self.fleet = FleetViewModel { rows: Vec::new() };
        self.dashboard = DashboardViewModel {
            total_devices: 0,
            degraded_devices: 0,
        };
        self.settings.plugin_rows.clear();
        self.device_detail.active_source_id = None;
        self.session = SessionViewModel::idle();
        self.diagnostics.host_error = None;
        self.diagnostics.control_summary = "control not started".into();
        self.diagnostics.grounding_summary = "grounding idle".into();
    }

    pub fn select_capture_source(&mut self, source_id: &str) {
        let Some(source) = self.device_detail.capture_source(source_id) else {
            return;
        };

        self.device_detail.active_source_id = Some(source.source_id.clone());
        self.session.selected_source = Some(source);
    }

    pub fn apply_runtime_snapshot(&mut self, snapshot: HostRuntimeSnapshot) {
        self.runtime_statuses = snapshot.statuses.clone();
        self.runtime_workspace = Some(snapshot.workspace.clone());
        self.preview_texture = None;
        self.sync_from_runtime();
    }

    fn sync_from_runtime(&mut self) {
        let statuses = &self.runtime_statuses;
        self.fleet = FleetViewModel::from_statuses(statuses);
        self.available_device_ids = self
            .fleet
            .rows
            .iter()
            .map(|row| row.device_id.clone())
            .collect();
        let summaries: Vec<_> = statuses
            .iter()
            .map(|status| status.summary().clone())
            .collect();
        self.dashboard = DashboardViewModel::from_sessions(&summaries);

        if self
            .selected_device_id
            .as_deref()
            .is_none_or(|selected| !self.available_device_ids.iter().any(|id| id == selected))
        {
            self.selected_device_id = self.available_device_ids.first().cloned();
        }

        self.settings.plugin_rows = statuses
            .iter()
            .flat_map(|status| {
                [
                    status.summary().capture_plugin.clone(),
                    status.summary().control_plugin.clone(),
                    status.summary().grounding_plugin.clone(),
                ]
            })
            .flatten()
            .collect();
        self.settings.plugin_rows.sort();
        self.settings.plugin_rows.dedup();

        self.sync_selected_workspace();
    }

    fn sync_selected_workspace(&mut self) {
        let Some(selected_device_id) = self.selected_device_id.as_deref() else {
            return;
        };
        let Some(status) = self
            .runtime_statuses
            .iter()
            .find(|status| status.summary().device_id == selected_device_id)
        else {
            return;
        };

        if let Some(workspace) = self
            .runtime_workspace
            .as_ref()
            .filter(|workspace| workspace.device_id == selected_device_id)
        {
            self.device_detail.device_name = workspace.summary.device_name.clone();
            self.device_detail.capture_sources = workspace
                .capture_sources
                .iter()
                .map(|source| CaptureSourceOption::new(&source.source_id, &source.display_name))
                .collect();
            self.device_detail.active_source_id = workspace.selected_source_id.clone();
            self.device_detail.control_checklist = ControlSetupChecklist {
                items: workspace.control_checklist.items.clone(),
            };

            let Some(source) = workspace
                .selected_source_id
                .as_deref()
                .and_then(|source_id| self.device_detail.capture_source(source_id))
            else {
                self.session = SessionViewModel::error("No runtime capture source selected");
                return;
            };

            self.diagnostics.host_error = status.operator_action().map(str::to_string);
            self.diagnostics.control_summary = format!(
                "{:?}: {}",
                workspace.control_phase, workspace.diagnostics.control_summary
            );
            self.diagnostics.grounding_summary = workspace
                .diagnostics
                .grounding_summary
                .clone()
                .unwrap_or_else(|| "grounding idle".into());

            self.session = match status.substate() {
                SessionSubstate::ControlReady | SessionSubstate::Streaming => {
                    if let Some(frame) = workspace.latest_frame.clone() {
                        SessionViewModel::streaming(source, frame)
                    } else {
                        SessionViewModel::starting()
                    }
                }
                SessionSubstate::Discovering
                | SessionSubstate::StartingCapture
                | SessionSubstate::StartingControl
                | SessionSubstate::Recovering => SessionViewModel::starting(),
                SessionSubstate::OperatorActionRequired
                | SessionSubstate::DegradedCapture
                | SessionSubstate::DegradedControl => SessionViewModel::error(
                    status
                        .operator_action()
                        .unwrap_or("Session requires operator intervention"),
                ),
                SessionSubstate::Stopped => SessionViewModel::idle(),
            };
            return;
        }

        self.device_detail.device_name = status.summary().device_name.clone();
        let source = capture_source_for_backend(status.backends().capture_backend.as_str());
        self.device_detail.capture_sources = vec![source.clone()];
        self.device_detail.active_source_id = Some(source.source_id.clone());
        self.device_detail.control_checklist = ControlSetupChecklist::for_pointer_mode();

        self.diagnostics.host_error = status.operator_action().map(str::to_string);
        self.diagnostics.control_summary =
            format!("control backend {}", status.backends().control_backend);
        self.diagnostics.grounding_summary = format!("session {:?}", status.substate());

        self.session = match status.substate() {
            SessionSubstate::ControlReady | SessionSubstate::Streaming => {
                SessionViewModel::streaming(
                    source,
                    ios_control_contracts::capture::VideoFrameDescriptor {
                        source_id: self
                            .device_detail
                            .active_source_id
                            .clone()
                            .unwrap_or_else(|| "window-helper-1".into()),
                        source_kind: if status
                            .backends()
                            .capture_backend
                            .starts_with("capture.window")
                        {
                            ios_control_contracts::capture::SourceKind::Window
                        } else {
                            ios_control_contracts::capture::SourceKind::DirectReceiver
                        },
                        width: 1280,
                        height: 720,
                        rotation_degrees: 0,
                        frame_index: 1,
                        health: ios_control_contracts::capture::FrameHealth::Healthy,
                    },
                )
            }
            SessionSubstate::Discovering
            | SessionSubstate::StartingCapture
            | SessionSubstate::StartingControl
            | SessionSubstate::Recovering => SessionViewModel::starting(),
            SessionSubstate::OperatorActionRequired
            | SessionSubstate::DegradedCapture
            | SessionSubstate::DegradedControl => SessionViewModel::error(
                status
                    .operator_action()
                    .unwrap_or("Session requires operator intervention"),
            ),
            SessionSubstate::Stopped => SessionViewModel::idle(),
        };
    }

    fn sync_preview_texture(&mut self, ctx: &egui::Context) {
        let Some(workspace) = self.runtime_workspace.as_ref() else {
            self.preview_texture = None;
            return;
        };
        let Some(stream) = workspace.capture_stream.as_ref() else {
            self.preview_texture = None;
            return;
        };
        let Ok(image) = color_image_from_slot(stream) else {
            self.preview_texture = None;
            return;
        };

        if let Some(texture) = self.preview_texture.as_mut() {
            texture.set(image, egui::TextureOptions::LINEAR);
        } else {
            self.preview_texture =
                Some(ctx.load_texture("session-preview", image, egui::TextureOptions::LINEAR));
        }
    }
}

fn capture_source_for_backend(backend: &str) -> CaptureSourceOption {
    if backend.starts_with("capture.window") {
        CaptureSourceOption::new("window-helper-1", "Operator Mirror")
    } else {
        CaptureSourceOption::new("direct-1", "Direct Receiver")
    }
}

impl eframe::App for HostDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut pending_action = SessionAction::None;
        let mut selected_device = None;
        let mut device_detail_action = DeviceDetailAction::None;

        let runtime_snapshot = if matches!(self.session.ui_state, SessionUiState::Streaming) {
            if let (Some(host_runtime), Some(device_id)) =
                (self.host_runtime.as_mut(), self.selected_device_id.clone())
            {
                host_runtime.refresh_session(&device_id).ok()
            } else {
                None
            }
        } else {
            None
        };
        if let Some(snapshot) = runtime_snapshot {
            self.apply_runtime_snapshot(snapshot);
        }

        self.sync_preview_texture(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            selected_device = dashboard::render(
                ui,
                &self.dashboard,
                &self.fleet,
                self.selected_device_id.as_deref(),
            );
            ui.separator();
            device_detail_action = device_detail::render(ui, &self.device_detail);
            ui.separator();
            pending_action = session_view::render(ui, &self.session, self.preview_texture.as_ref());
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

        if let Some(device_id) = selected_device {
            self.select_device(&device_id);
            ctx.request_repaint();
        }

        match device_detail_action {
            DeviceDetailAction::None => {}
            DeviceDetailAction::SelectCaptureSource(source_id) => {
                self.select_capture_source(&source_id);
                ctx.request_repaint();
            }
        }

        match pending_action {
            SessionAction::None => {}
            SessionAction::Start => {
                self.request_start_session();
                ctx.request_repaint();
            }
            SessionAction::Stop => {
                self.stop_session();
                ctx.request_repaint();
            }
        }
    }
}
