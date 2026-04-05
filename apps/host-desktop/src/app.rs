use eframe::egui;
use ios_control_contracts::session::{DeviceSessionStatus, SessionSubstate};
use std::time::{Duration, Instant};

use crate::panels::device_detail::{
    CaptureSourceOption, ControlSetupChecklist, DeviceDetailAction,
};
use crate::panels::session_view::SessionAction;
use crate::panels::{dashboard, device_detail, diagnostics, session_view, settings, startup};
use crate::preferences::{HostPreferences, HostPreferencesStore};
use crate::preview::color_image_from_slot;
use crate::runtime::{HostRuntime, HostRuntimeConfig, HostRuntimeSnapshot, RuntimeWorkspaceState};
use crate::bootstrap::capability_probe::startup_from_plugin_paths;
use crate::view_models::dashboard::DashboardViewModel;
use crate::view_models::device_detail::DeviceDetailViewModel;
use crate::view_models::diagnostics::DiagnosticsViewModel;
use crate::view_models::fleet::FleetViewModel;
use crate::view_models::session::SessionViewModel;
use crate::view_models::settings::SettingsViewModel;
use crate::view_models::startup::StartupViewModel;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RestoredSourcePreference {
    device_id: String,
    source_id: String,
}

pub struct HostDesktopApp {
    pub available_device_ids: Vec<String>,
    pub selected_device_id: Option<String>,
    pub fleet: FleetViewModel,
    runtime_statuses: Vec<DeviceSessionStatus>,
    host_runtime: Option<HostRuntime>,
    runtime_workspace: Option<RuntimeWorkspaceState>,
    preferences_store: Option<HostPreferencesStore>,
    preferences: HostPreferences,
    restored_source_preference: Option<RestoredSourcePreference>,
    manual_source_selection_device_id: Option<String>,
    next_runtime_refresh_at: Option<Instant>,
    runtime_refresh_device_id: Option<String>,
    preview_texture: Option<egui::TextureHandle>,
    pub dashboard: DashboardViewModel,
    pub device_detail: DeviceDetailViewModel,
    pub session: SessionViewModel,
    pub diagnostics: DiagnosticsViewModel,
    pub settings: SettingsViewModel,
    pub startup: StartupViewModel,
}

impl HostDesktopApp {
    const RUNTIME_REFRESH_POLL_INTERVAL: Duration = Duration::from_millis(200);

    pub fn new() -> Self {
        Self {
            available_device_ids: Vec::new(),
            selected_device_id: None,
            fleet: FleetViewModel { rows: Vec::new() },
            runtime_statuses: Vec::new(),
            host_runtime: None,
            runtime_workspace: None,
            preferences_store: None,
            preferences: HostPreferences::default(),
            restored_source_preference: None,
            manual_source_selection_device_id: None,
            next_runtime_refresh_at: None,
            runtime_refresh_device_id: None,
            preview_texture: None,
            dashboard: DashboardViewModel {
                total_devices: 0,
                degraded_devices: 0,
            },
            device_detail: DeviceDetailViewModel {
                device_name: "No device selected".into(),
                capture_sources: Vec::new(),
                active_source_id: None,
                control_checklist: ControlSetupChecklist { items: Vec::new() },
            },
            session: SessionViewModel::idle(),
            diagnostics: DiagnosticsViewModel {
                host_error: None,
                control_summary: "control not started".into(),
                grounding_summary: "grounding idle".into(),
            },
            settings: SettingsViewModel {
                plugin_rows: Vec::new(),
            },
            startup: StartupViewModel::blocked("Blocked: no usable device path yet"),
        }
    }

    pub fn demo() -> Self {
        Self::new()
    }

    pub fn with_runtime(config: HostRuntimeConfig) -> Self {
        let mut app = Self::new();
        app.startup = startup_from_plugin_paths(&config.plugin_paths);
        app.host_runtime =
            Some(HostRuntime::new(config).expect("host runtime should initialize successfully"));
        app
    }

    pub fn with_runtime_and_preferences(
        config: HostRuntimeConfig,
        store: HostPreferencesStore,
    ) -> Self {
        let mut app = Self::with_runtime(config);
        let preferences = store.load().unwrap_or_default();
        let restored_source_preference = preferences
            .selected_device_id
            .as_ref()
            .zip(preferences.selected_source_id.as_ref())
            .map(|(device_id, source_id)| RestoredSourcePreference {
                device_id: device_id.clone(),
                source_id: source_id.clone(),
            });
        app.selected_device_id = preferences.selected_device_id.clone();
        app.preferences = preferences;
        app.restored_source_preference = restored_source_preference;
        app.preferences_store = Some(store);
        app
    }

    pub fn replace_runtime_statuses(&mut self, statuses: Vec<DeviceSessionStatus>) {
        self.runtime_workspace = None;
        self.next_runtime_refresh_at = None;
        self.runtime_refresh_device_id = None;
        self.runtime_statuses = statuses;
        self.sync_from_runtime();
    }

    fn persist_preferences(&mut self) {
        if let Some(store) = self.preferences_store.as_ref() {
            if let Err(error) = store.save(&self.preferences) {
                eprintln!("warning: failed to save host preferences: {error}");
            }
        }
    }

    fn clear_restored_source_preference_for_device(&mut self, device_id: &str) {
        if self
            .restored_source_preference
            .as_ref()
            .is_some_and(|pref| pref.device_id == device_id)
        {
            self.restored_source_preference = None;
        }
    }

    pub fn select_device(&mut self, device_id: &str) {
        let device_changed = self.selected_device_id.as_deref() != Some(device_id);
        self.selected_device_id = Some(device_id.into());
        if device_changed {
            self.preferences.selected_device_id = Some(device_id.into());
            self.preferences.selected_source_id = None;
            self.clear_restored_source_preference_for_device(device_id);
            self.manual_source_selection_device_id = None;
            self.persist_preferences();
        }
        self.next_runtime_refresh_at = None;
        self.runtime_refresh_device_id = None;
        self.sync_selected_workspace();
    }

    pub fn start_runtime_session_on_launch(&mut self) {
        if self.selected_device_id.is_some() {
            self.request_start_session();
        }
    }

    pub fn request_start_session(&mut self) {
        if self.host_runtime.is_none() {
            let message = "Host runtime unavailable";
            self.session = SessionViewModel::error(message);
            self.diagnostics.host_error = Some(message.into());
            self.diagnostics.control_summary = "control blocked".into();
            self.diagnostics.grounding_summary = "grounding blocked".into();
            return;
        }

        let Some(device_id) = self
            .selected_device_id
            .clone()
            .or_else(|| self.available_device_ids.first().cloned())
        else {
            self.session = SessionViewModel::error("No device selected");
            return;
        };

        let manual_source = if self.manual_source_selection_device_id.as_deref()
            == Some(device_id.as_str())
        {
            self.device_detail.active_source_id.clone()
        } else {
            None
        };
        let workspace_source = self
            .runtime_workspace
            .as_ref()
            .filter(|workspace| workspace.device_id == device_id)
            .and_then(|workspace| workspace.selected_source_id.clone());
        let explicit_source = manual_source.or(workspace_source);
        let mut restored_source = if explicit_source.is_none() {
            self.restored_source_preference
                .as_ref()
                .filter(|pref| pref.device_id == device_id)
                .map(|pref| pref.source_id.clone())
        } else {
            None
        };
        if let Some(restored_source_id) = restored_source.as_ref() {
            let source_known_unavailable = !self.device_detail.capture_sources.is_empty()
                && !self
                    .device_detail
                    .capture_sources
                    .iter()
                    .any(|source| source.source_id == *restored_source_id);
            if source_known_unavailable {
                self.clear_restored_source_preference_for_device(&device_id);
                self.preferences.selected_device_id = Some(device_id.clone());
                self.preferences.selected_source_id = None;
                self.persist_preferences();
                restored_source = None;
            }
        }
        let inferred_source = if explicit_source.is_none() && restored_source.is_none() {
            self.device_detail.active_source_id.clone()
        } else {
            None
        };
        let mut selected_source_id = explicit_source
            .clone()
            .or(restored_source.clone())
            .or(inferred_source);
        self.selected_device_id = Some(device_id.clone());
        self.session = SessionViewModel::starting();
        self.diagnostics.host_error = None;
        self.diagnostics.control_summary = "control bootstrapping".into();
        self.diagnostics.grounding_summary = "grounding bootstrapping".into();

        for attempt in 0..2 {
            let start_result = self
                .host_runtime
                .as_mut()
                .expect("host runtime should be present")
                .start_session(
                    &device_id,
                    &self.device_detail.device_name,
                    selected_source_id.clone(),
                );

            match start_result {
                Ok(snapshot) => {
                    self.apply_runtime_snapshot(snapshot);
                    self.clear_restored_source_preference_for_device(&device_id);
                    self.preferences.selected_device_id = self.selected_device_id.clone();
                    self.preferences.selected_source_id =
                        self.device_detail.active_source_id.clone();
                    self.persist_preferences();
                    return;
                }
                Err(error) => {
                    let message = error.to_string();
                    let stale_restored_source = restored_source.as_deref().is_some_and(|source_id| {
                        attempt == 0
                            && message.contains("requested capture source")
                            && message.contains(source_id)
                            && message.contains("unavailable")
                    });
                    if stale_restored_source {
                        self.clear_restored_source_preference_for_device(&device_id);
                        self.preferences.selected_source_id = None;
                        self.persist_preferences();
                        selected_source_id = None;
                        continue;
                    }
                    self.session = SessionViewModel::error(&message);
                    self.diagnostics.host_error = Some(message);
                    self.diagnostics.control_summary = "control blocked".into();
                    self.diagnostics.grounding_summary = "grounding blocked".into();
                    return;
                }
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
        self.next_runtime_refresh_at = None;
        self.runtime_refresh_device_id = None;
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
        self.restored_source_preference = None;
        self.manual_source_selection_device_id = None;
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
        self.session.selected_source = Some(source.clone());
        self.manual_source_selection_device_id = self.selected_device_id.clone();
        if let Some(device_id) = self.selected_device_id.clone() {
            self.clear_restored_source_preference_for_device(device_id.as_str());
        }
        self.preferences.selected_device_id = self.selected_device_id.clone();
        self.preferences.selected_source_id = Some(source.source_id);
        self.persist_preferences();
    }

    pub fn apply_runtime_snapshot(&mut self, snapshot: HostRuntimeSnapshot) {
        self.runtime_statuses = snapshot.statuses.clone();
        self.runtime_workspace = Some(snapshot.workspace.clone());
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
        let Some(selected_device_id) = self.selected_device_id.clone() else {
            return;
        };
        let Some(status) = self
            .runtime_statuses
            .iter()
            .find(|status| status.summary().device_id == selected_device_id)
            .cloned()
        else {
            return;
        };

        if let Some(workspace) = self
            .runtime_workspace
            .as_ref()
            .filter(|workspace| workspace.device_id == selected_device_id)
            .cloned()
        {
            self.device_detail.device_name = workspace.summary.device_name.clone();
            self.device_detail.capture_sources = workspace
                .capture_sources
                .iter()
                .map(|source| CaptureSourceOption::new(&source.source_id, &source.display_name))
                .collect();
            self.device_detail.active_source_id = workspace.selected_source_id.clone();
            self.clear_restored_source_preference_for_device(selected_device_id.as_str());
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
                        SessionViewModel::streaming_without_frame(source)
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
                SessionViewModel::streaming_without_frame(source)
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

    fn selected_runtime_session_is_streaming(&self) -> bool {
        self.selected_runtime_streaming_device_id().is_some()
    }

    fn selected_runtime_streaming_device_id(&self) -> Option<&str> {
        let Some(device_id) = self.selected_device_id.as_deref() else {
            return None;
        };
        let workspace_matches = self
            .runtime_workspace
            .as_ref()
            .is_some_and(|workspace| workspace.device_id == device_id);
        if !workspace_matches {
            return None;
        }
        let streaming = self
            .runtime_statuses
            .iter()
            .find(|status| status.summary().device_id == device_id)
            .is_some_and(|status| {
                matches!(
                    status.substate(),
                    SessionSubstate::ControlReady | SessionSubstate::Streaming
                )
            });
        if streaming {
            Some(device_id)
        } else {
            None
        }
    }

    fn poll_runtime_refresh_if_due(&mut self, now: Instant) {
        let Some(device_id) = self
            .selected_runtime_streaming_device_id()
            .map(str::to_string)
        else {
            self.next_runtime_refresh_at = None;
            self.runtime_refresh_device_id = None;
            return;
        };

        if self.runtime_refresh_device_id.as_deref() != Some(device_id.as_str()) {
            self.runtime_refresh_device_id = Some(device_id.clone());
            self.next_runtime_refresh_at = None;
        }

        if self.next_runtime_refresh_at.is_some_and(|next| now < next) {
            return;
        }
        self.next_runtime_refresh_at = Some(now + Self::RUNTIME_REFRESH_POLL_INTERVAL);

        let Some(host_runtime) = self.host_runtime.as_mut() else {
            self.diagnostics.host_error = Some(format!(
                "Runtime refresh failed for {device_id}: host runtime unavailable"
            ));
            return;
        };

        match host_runtime.refresh_session(&device_id) {
            Ok(snapshot) => self.apply_runtime_snapshot(snapshot),
            Err(error) => {
                self.diagnostics.host_error =
                    Some(format!("Runtime refresh failed for {device_id}: {error}"));
            }
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

        if self.selected_runtime_session_is_streaming() {
            ctx.request_repaint_after(Self::RUNTIME_REFRESH_POLL_INTERVAL);
        }
        self.poll_runtime_refresh_if_due(Instant::now());

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
            ui.separator();
            startup::render(ui, &self.startup);
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

#[cfg(test)]
mod tests {
    use super::*;
    use ios_control_contracts::control::ControlSessionPhase;
    use ios_control_contracts::plugin::PluginHealth;
    use ios_control_contracts::session::{
        BackendSelection, DeviceSessionStatus, DeviceSessionSummary, SessionPhase,
    };
    use ios_control_session_orchestrator::{PluginPaths, SessionDiagnostics};
    use egui::{Color32, ColorImage};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn app_with_runtime_without_active_session() -> HostDesktopApp {
        HostDesktopApp::with_runtime(HostRuntimeConfig {
            plugin_paths: PluginPaths {
                capture: PathBuf::from("missing-capture-plugin"),
                control_ble: PathBuf::from("missing-control-ble-plugin"),
                control_fallback: PathBuf::from("missing-control-fallback-plugin"),
                grounding: None,
            },
        })
    }

    fn streaming_snapshot_without_frame() -> HostRuntimeSnapshot {
        let status = DeviceSessionStatus::new(
            DeviceSessionSummary {
                device_id: "device-1".into(),
                device_name: "Alpha".into(),
                phase: SessionPhase::Streaming,
                plugin_health: PluginHealth::Healthy,
                capture_plugin: Some("capture.window.helper".into()),
                control_plugin: Some("control.ble".into()),
                grounding_plugin: Some("grounding.core".into()),
            },
            SessionSubstate::Streaming,
            BackendSelection {
                capture_backend: "capture.window.helper".into(),
                control_backend: "control.ble".into(),
            },
            None,
        )
        .expect("valid session status");

        HostRuntimeSnapshot {
            statuses: vec![status.clone()],
            workspace: RuntimeWorkspaceState {
                device_id: "device-1".into(),
                summary: status.summary().clone(),
                capture_sources: vec![ios_control_contracts::capture::VideoSource {
                    source_id: "window-helper-1".into(),
                    display_name: "Operator Mirror".into(),
                    kind: ios_control_contracts::capture::SourceKind::Window,
                }],
                capture_stream: None,
                latest_frame: None,
                selected_source_id: Some("window-helper-1".into()),
                control_checklist: ios_control_contracts::control::ControlSetupChecklist {
                    items: vec!["Pair the device".into()],
                },
                control_phase: ControlSessionPhase::Connected,
                execution_observed_change: Some(true),
                diagnostics: SessionDiagnostics {
                    control_phase: ControlSessionPhase::Connected,
                    control_summary: "control ready".into(),
                    grounding_summary: Some("selected pointer plan".into()),
                },
            },
        }
    }

    #[test]
    fn update_requests_polling_repaint_and_surfaces_refresh_errors() {
        let mut app = app_with_runtime_without_active_session();
        app.apply_runtime_snapshot(streaming_snapshot_without_frame());
        let mut frame = eframe::Frame::_new_kittest();
        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            eframe::App::update(&mut app, ctx, &mut frame);
        });

        assert!(app
            .diagnostics
            .host_error
            .as_deref()
            .is_some_and(|error| error.contains("Runtime refresh failed")));
        let root = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .expect("root viewport output");
        assert!(root.repaint_delay <= HostDesktopApp::RUNTIME_REFRESH_POLL_INTERVAL);
    }

    #[test]
    fn runtime_refresh_attempts_are_throttled_to_poll_interval() {
        let mut app = app_with_runtime_without_active_session();
        app.apply_runtime_snapshot(streaming_snapshot_without_frame());

        let start = Instant::now();
        app.poll_runtime_refresh_if_due(start);
        assert!(app
            .diagnostics
            .host_error
            .as_deref()
            .is_some_and(|error| error.contains("Runtime refresh failed")));

        app.diagnostics.host_error = None;
        app.poll_runtime_refresh_if_due(start + Duration::from_millis(10));
        assert_eq!(app.diagnostics.host_error, None);

        app.poll_runtime_refresh_if_due(
            start + HostDesktopApp::RUNTIME_REFRESH_POLL_INTERVAL + Duration::from_millis(1),
        );
        assert!(app
            .diagnostics
            .host_error
            .as_deref()
            .is_some_and(|error| error.contains("Runtime refresh failed")));
    }

    #[test]
    fn apply_runtime_snapshot_preserves_existing_preview_texture() {
        let mut app = HostDesktopApp::new();
        let ctx = egui::Context::default();
        let mut texture = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            texture = Some(ctx.load_texture(
                "preserve-preview-texture",
                ColorImage::new([1, 1], Color32::WHITE),
                egui::TextureOptions::LINEAR,
            ));
        });
        app.preview_texture = texture;
        assert!(app.preview_texture.is_some());

        app.apply_runtime_snapshot(streaming_snapshot_without_frame());

        assert!(app.preview_texture.is_some());
    }
}
