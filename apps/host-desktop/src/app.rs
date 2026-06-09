use eframe::egui;
use ios_control_contracts::session::{DeviceSessionStatus, SessionSubstate};
use ios_control_session_orchestrator::CaptureBackend;
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use crate::bootstrap::capability_probe::startup_from_plugin_paths;
use crate::inventory::collect_inventory_snapshot;
use crate::inventory::model::{
    InventoryDevice, InventoryEvidenceSource, InventorySnapshot, Sessionability,
};
use crate::logging::HostLogWriter;
use crate::panels::device_detail::{CaptureSourceOption, ControlSetupChecklist};
use crate::panels::launcher::LauncherAction;
use crate::panels::session_view::SessionAction;
use crate::panels::{launcher, session_view, settings};
use crate::preferences::{
    HostPreferences, HostPreferencesStore, KnownDevicePreference, MAX_DIRECT_PREVIEW_FPS,
    MAX_DIRECT_PREVIEW_HEIGHT, MIN_DIRECT_PREVIEW_FPS, MIN_DIRECT_PREVIEW_HEIGHT,
};
use crate::preview::{
    color_image_from_slot, detect_visible_content_bounds, PreviewContentBounds, PreviewInputBridge,
};
use crate::runtime::{
    DirectPreviewConfig, HostRuntime, HostRuntimeConfig, HostRuntimeSnapshot, RuntimeWorkspaceState,
};
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
    inventory_snapshot: InventorySnapshot,
    inventory_missed_refreshes: BTreeMap<String, u8>,
    runtime_statuses: Vec<DeviceSessionStatus>,
    host_runtime: Option<HostRuntime>,
    runtime_config: Option<HostRuntimeConfig>,
    runtime_workspace: Option<RuntimeWorkspaceState>,
    preferences_store: Option<HostPreferencesStore>,
    host_log_writer: Option<HostLogWriter>,
    preferences: HostPreferences,
    restored_source_preference: Option<RestoredSourcePreference>,
    manual_source_selection_device_id: Option<String>,
    pending_start_device_id: Option<String>,
    next_inventory_refresh_at: Option<Instant>,
    next_runtime_refresh_at: Option<Instant>,
    runtime_refresh_device_id: Option<String>,
    preview_texture: Option<egui::TextureHandle>,
    preview_content_bounds: Option<PreviewContentBounds>,
    preview_input_bridge: PreviewInputBridge,
    session_window_open: bool,
    session_window_deferred_until_streaming: bool,
    session_window_focus_requested: bool,
    session_window_device_id: Option<String>,
    session_window_auto_sized_frame: Option<(u32, u32, u16, Option<PreviewContentBounds>)>,
    session_window_resize_pending: bool,
    pub dashboard: DashboardViewModel,
    pub device_detail: DeviceDetailViewModel,
    pub session: SessionViewModel,
    pub diagnostics: DiagnosticsViewModel,
    pub settings: SettingsViewModel,
    pub startup: StartupViewModel,
}

impl HostDesktopApp {
    const INVENTORY_REFRESH_POLL_INTERVAL: Duration = Duration::from_secs(2);
    const INVENTORY_MISSED_REFRESH_GRACE: u8 = 3;
    const RUNTIME_REFRESH_POLL_INTERVAL: Duration = Duration::from_millis(40);
    const WAITING_FOR_MIRROR_REFRESH_POLL_INTERVAL: Duration = Duration::from_millis(250);
    const DIRECT_RECEIVER_DEVICE_ID: &str = "direct-receiver";
    const DIRECT_RECEIVER_DEVICE_NAME: &str = "Direct Receiver";
    const DIRECT_RECEIVER_SOURCE_ID: &str = "direct-1";

    pub fn new() -> Self {
        Self {
            available_device_ids: Vec::new(),
            selected_device_id: None,
            fleet: FleetViewModel { rows: Vec::new() },
            inventory_snapshot: InventorySnapshot::default(),
            inventory_missed_refreshes: BTreeMap::new(),
            runtime_statuses: Vec::new(),
            host_runtime: None,
            runtime_config: None,
            runtime_workspace: None,
            preferences_store: None,
            host_log_writer: None,
            preferences: HostPreferences::default(),
            restored_source_preference: None,
            manual_source_selection_device_id: None,
            pending_start_device_id: None,
            next_inventory_refresh_at: None,
            next_runtime_refresh_at: None,
            runtime_refresh_device_id: None,
            preview_texture: None,
            preview_content_bounds: None,
            preview_input_bridge: PreviewInputBridge::default(),
            session_window_open: false,
            session_window_deferred_until_streaming: false,
            session_window_focus_requested: false,
            session_window_device_id: None,
            session_window_auto_sized_frame: None,
            session_window_resize_pending: false,
            dashboard: DashboardViewModel {
                total_devices: 0,
                degraded_devices: 0,
            },
            device_detail: DeviceDetailViewModel {
                device_name: "No device selected".into(),
                capture_sources: Vec::new(),
                active_source_id: None,
                control_checklist: ControlSetupChecklist { items: Vec::new() },
                inventory_notes: Vec::new(),
            },
            session: SessionViewModel::idle(),
            diagnostics: DiagnosticsViewModel {
                host_error: None,
                control_summary: "control not started".into(),
                grounding_summary: "grounding idle".into(),
                startup_probe_runs: 0,
                inventory_refreshes: 0,
                inventory_rows: 0,
                inventory_startable_rows: 0,
                inventory_blocked_rows: 0,
                session_start_attempts: 0,
                session_start_successes: 0,
                session_start_failures: 0,
                log_lines: Vec::new(),
            },
            settings: SettingsViewModel { rows: Vec::new() },
            startup: StartupViewModel::blocked("Blocked: no usable device path yet"),
        }
    }

    pub fn demo() -> Self {
        Self::new()
    }

    pub fn with_runtime(config: HostRuntimeConfig) -> Self {
        let mut app = Self::new();
        let startup = startup_from_plugin_paths(&config.plugin_paths);
        app.record_startup_view(&startup);
        app.startup = startup;
        app.runtime_config = Some(config.clone());
        app.host_runtime =
            Some(HostRuntime::new(config).expect("host runtime should initialize successfully"));
        app.refresh_inventory();
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
        app.preview_input_bridge
            .set_pointer_long_axis_units(app.preferences.ble_pointer_long_axis_units);
        app.sync_direct_preview_config_to_runtime();
        app.restored_source_preference = restored_source_preference;
        app.install_log_writer_from_preferences_path(store.path());
        app.settings = SettingsViewModel::from_preferences_path(Some(store.path()));
        app.preferences_store = Some(store);
        app.refresh_inventory();
        if app.preferences.selected_device_id.is_some() {
            app.selected_device_id = app.preferences.selected_device_id.clone();
            app.sync_selected_workspace();
        }
        app
    }

    pub fn replace_runtime_statuses(&mut self, statuses: Vec<DeviceSessionStatus>) {
        self.runtime_workspace = None;
        self.next_inventory_refresh_at = None;
        self.next_runtime_refresh_at = None;
        self.runtime_refresh_device_id = None;
        self.runtime_statuses = statuses;
        self.sync_from_inventory_and_runtime();
    }

    pub fn apply_startup_view(&mut self, startup: StartupViewModel) {
        self.record_startup_view(&startup);
        self.startup = startup;
    }

    pub fn apply_inventory_snapshot(&mut self, snapshot: InventorySnapshot) {
        let snapshot = self.stabilize_inventory_snapshot(snapshot);
        self.record_inventory_snapshot(&snapshot);
        self.inventory_snapshot = snapshot;
        self.sync_from_inventory_and_runtime();
    }

    fn stabilize_inventory_snapshot(
        &mut self,
        mut snapshot: InventorySnapshot,
    ) -> InventorySnapshot {
        for device in &snapshot.devices {
            self.inventory_missed_refreshes.remove(&device.inventory_id);
        }

        let current_ids = snapshot
            .devices
            .iter()
            .map(|device| device.inventory_id.clone())
            .collect::<BTreeSet<_>>();
        let mut expired = Vec::new();

        for previous in &self.inventory_snapshot.devices {
            if current_ids.contains(&previous.inventory_id) || !is_transient_bluetooth_row(previous)
            {
                continue;
            }

            let missed = self
                .inventory_missed_refreshes
                .entry(previous.inventory_id.clone())
                .or_insert(0);
            if *missed >= Self::INVENTORY_MISSED_REFRESH_GRACE {
                expired.push(previous.inventory_id.clone());
                continue;
            }

            *missed += 1;
            snapshot.devices.push(previous.clone());
        }

        for device_id in expired {
            self.inventory_missed_refreshes.remove(&device_id);
        }

        snapshot
    }

    fn refresh_inventory(&mut self) {
        let Some(config) = self.runtime_config.as_ref() else {
            self.apply_inventory_snapshot(InventorySnapshot::default());
            return;
        };
        self.apply_inventory_snapshot(collect_inventory_snapshot(
            &config.plugin_paths,
            &self.preferences,
        ));
    }

    fn persist_preferences(&mut self) {
        if let Some(store) = self.preferences_store.as_ref() {
            if let Err(error) = store.save(&self.preferences) {
                eprintln!("warning: failed to save host preferences: {error}");
            }
        }
    }

    fn install_log_writer_from_preferences_path(&mut self, preferences_path: &std::path::Path) {
        match HostLogWriter::from_preferences_path(preferences_path) {
            Ok(writer) => {
                self.host_log_writer = Some(writer);
                for line in self.diagnostics.log_lines.clone() {
                    self.append_host_log_line(&line);
                }
            }
            Err(error) => {
                eprintln!("warning: failed to initialize host log writer: {error}");
                self.diagnostics
                    .record_host_log_line(format!("host log file unavailable: {error}"));
            }
        }
    }

    fn append_host_log_line(&mut self, line: &str) {
        let Some(writer) = self.host_log_writer.as_ref() else {
            return;
        };

        if let Err(error) = writer.append_line(line) {
            eprintln!("warning: failed to append host log line: {error}");
            self.host_log_writer = None;
            self.diagnostics
                .record_host_log_line(format!("host log append failed: {error}"));
        }
    }

    fn set_ble_pointer_long_axis_units(&mut self, units: Option<u32>) {
        self.preview_input_bridge.set_pointer_long_axis_units(units);
        self.preferences.ble_pointer_long_axis_units =
            self.preview_input_bridge.pointer_long_axis_units();
        self.persist_preferences();

        let value = self
            .preferences
            .ble_pointer_long_axis_units
            .map(|value| value.to_string())
            .unwrap_or_else(|| "auto".into());
        self.append_host_log_line(&format!("BLE pointer scale set to {value}"));
    }

    fn sync_direct_preview_config_to_runtime(&mut self) {
        let config = DirectPreviewConfig::new(
            self.preferences.direct_preview_height(),
            self.preferences.direct_preview_fps(),
        );
        if let Some(runtime) = self.host_runtime.as_mut() {
            runtime.set_direct_preview_config(config);
        }
    }

    fn set_direct_preview_fps(&mut self, fps: Option<u32>) {
        self.preferences.direct_preview_fps =
            fps.map(|value| value.clamp(MIN_DIRECT_PREVIEW_FPS, MAX_DIRECT_PREVIEW_FPS));
        self.sync_direct_preview_config_to_runtime();
        self.persist_preferences();
        self.append_host_log_line(&format!(
            "Direct preview FPS set to {}",
            self.preferences.direct_preview_fps()
        ));
    }

    fn set_direct_preview_height(&mut self, height: Option<u32>) {
        self.preferences.direct_preview_height =
            height.map(|value| value.clamp(MIN_DIRECT_PREVIEW_HEIGHT, MAX_DIRECT_PREVIEW_HEIGHT));
        self.sync_direct_preview_config_to_runtime();
        self.persist_preferences();
        self.append_host_log_line(&format!(
            "Direct preview height set to {}",
            self.preferences.direct_preview_height()
        ));
    }

    fn record_startup_view(&mut self, startup: &StartupViewModel) {
        let line = self.diagnostics.record_startup_view(startup);
        self.append_host_log_line(&line);
    }

    fn record_inventory_snapshot(&mut self, snapshot: &InventorySnapshot) {
        let line = self.diagnostics.record_inventory_snapshot(snapshot);
        self.append_host_log_line(&line);
    }

    fn record_session_start_attempt(&mut self, device_id: &str, source_id: Option<&str>) {
        let line = self
            .diagnostics
            .record_session_start_attempt(device_id, source_id);
        self.append_host_log_line(&line);
    }

    fn record_session_start_success(&mut self, device_id: &str, source_id: Option<&str>) {
        let line = self
            .diagnostics
            .record_session_start_success(device_id, source_id);
        self.append_host_log_line(&line);
    }

    fn record_session_start_failure(&mut self, device_id: Option<&str>, error: &str) {
        let line = self
            .diagnostics
            .record_session_start_failure(device_id, error);
        self.append_host_log_line(&line);
    }

    fn remember_known_device(
        &mut self,
        device_id: &str,
        device_name: &str,
        source_id: Option<String>,
        stable_id: Option<String>,
    ) {
        if let Some(existing) = self
            .preferences
            .known_devices
            .iter_mut()
            .find(|known| known.known_device_id == device_id)
        {
            existing.display_name = device_name.into();
            existing.last_source_id = source_id;
            if stable_id.is_some() {
                existing.stable_id = stable_id;
            }
            return;
        }

        self.preferences.known_devices.push(KnownDevicePreference {
            known_device_id: device_id.into(),
            display_name: device_name.into(),
            stable_id,
            last_source_id: source_id,
        });
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

    pub fn set_pending_start_device(&mut self, device_id: impl Into<String>) {
        self.pending_start_device_id = Some(device_id.into());
        self.try_start_pending_device();
    }

    pub fn request_open_selected_device_session(&mut self) {
        let Some(device_id) = self
            .selected_device_id
            .clone()
            .or_else(|| self.available_device_ids.first().cloned())
        else {
            self.session = SessionViewModel::error("No device selected");
            self.session_window_open = true;
            self.session_window_deferred_until_streaming = false;
            self.session_window_focus_requested = true;
            self.session_window_device_id = None;
            self.session_window_auto_sized_frame = None;
            return;
        };

        self.session_window_device_id = Some(device_id.clone());
        self.session_window_auto_sized_frame = None;

        let launcher = FleetViewModel::for_launcher(
            &self.inventory_snapshot.devices,
            self.startup.direct_receiver.available,
            &self.runtime_statuses,
        );
        let startable = launcher
            .rows
            .iter()
            .find(|row| row.device_id == device_id)
            .is_some_and(|row| row.start_enabled);

        if startable {
            self.session_window_open = true;
            self.session_window_deferred_until_streaming = false;
            self.session_window_focus_requested = true;
            self.session_window_auto_sized_frame = None;
            self.request_start_direct_session_for_device(&device_id);
            self.reconcile_session_window_state();
            return;
        }

        self.session_window_open = true;
        self.session_window_deferred_until_streaming = false;
        self.session_window_focus_requested = true;
        self.session_window_auto_sized_frame = None;
        if let Some(device) = self
            .inventory_snapshot
            .devices
            .iter()
            .find(|device| device.inventory_id == device_id)
            .cloned()
        {
            self.apply_inventory_detail(device);
        }
    }

    pub fn session_window_is_visible(&self) -> bool {
        self.session_window_open
    }

    pub fn request_start_session(&mut self) {
        if self.host_runtime.is_none() {
            let message = "Host runtime unavailable";
            let selected_device_id = self.selected_device_id.clone();
            self.session = SessionViewModel::error(message);
            self.diagnostics.host_error = Some(message.into());
            self.diagnostics.control_summary = "control blocked".into();
            self.diagnostics.grounding_summary = "grounding blocked".into();
            self.record_session_start_failure(selected_device_id.as_deref(), message);
            return;
        }

        let Some(device_id) = self
            .selected_device_id
            .clone()
            .or_else(|| self.available_device_ids.first().cloned())
        else {
            self.session = SessionViewModel::error("No device selected");
            self.record_session_start_failure(None, "No device selected");
            return;
        };

        let manual_source =
            if self.manual_source_selection_device_id.as_deref() == Some(device_id.as_str()) {
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
        self.record_session_start_attempt(&device_id, selected_source_id.as_deref());
        let selected_device_stable_id = self
            .inventory_snapshot
            .devices
            .iter()
            .find(|device| device.inventory_id == device_id)
            .and_then(|device| device.stable_id.clone());
        self.selected_device_id = Some(device_id.clone());
        self.session = SessionViewModel::starting();
        self.session_window_auto_sized_frame = None;
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
                    CaptureBackend::Window,
                );

            match start_result {
                Ok(snapshot) => {
                    self.remember_known_device(
                        &device_id,
                        &snapshot.workspace.summary.device_name,
                        snapshot.workspace.selected_source_id.clone(),
                        selected_device_stable_id.clone(),
                    );
                    self.record_session_start_success(
                        &device_id,
                        snapshot.workspace.selected_source_id.as_deref(),
                    );
                    self.apply_runtime_snapshot(snapshot);
                    self.clear_restored_source_preference_for_device(&device_id);
                    self.preferences.selected_device_id = self.selected_device_id.clone();
                    self.preferences.selected_source_id =
                        self.device_detail.active_source_id.clone();
                    self.persist_preferences();
                    self.refresh_inventory();
                    return;
                }
                Err(error) => {
                    let message = error.to_string();
                    let stale_restored_source =
                        restored_source.as_deref().is_some_and(|source_id| {
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
                    self.diagnostics.host_error = Some(message.clone());
                    self.diagnostics.control_summary = "control blocked".into();
                    self.diagnostics.grounding_summary = "grounding blocked".into();
                    self.record_session_start_failure(Some(&device_id), &message);
                    return;
                }
            }
        }
    }

    fn request_start_direct_session_for_device(&mut self, device_id: &str) {
        self.selected_device_id = Some(device_id.to_string());
        self.next_runtime_refresh_at = None;
        self.runtime_refresh_device_id = None;

        let device_name = self
            .inventory_snapshot
            .devices
            .iter()
            .find(|device| device.inventory_id == device_id)
            .map(|device| device.display_name.clone())
            .unwrap_or_else(|| self.device_detail.device_name.clone());

        if self.host_runtime.is_none() {
            let message = "Host runtime unavailable";
            self.session = SessionViewModel::error(message);
            self.diagnostics.host_error = Some(message.into());
            self.diagnostics.control_summary = "control blocked".into();
            self.diagnostics.grounding_summary = "grounding blocked".into();
            self.record_session_start_failure(Some(device_id), message);
            return;
        }

        self.session = SessionViewModel::starting();
        self.session_window_auto_sized_frame = None;
        self.diagnostics.host_error = None;
        self.diagnostics.control_summary = "control bootstrapping".into();
        self.diagnostics.grounding_summary = "grounding bootstrapping".into();
        self.record_session_start_attempt(device_id, Some(Self::DIRECT_RECEIVER_SOURCE_ID));

        match self
            .host_runtime
            .as_mut()
            .expect("host runtime should be present")
            .start_session(
                device_id,
                &device_name,
                Some(Self::DIRECT_RECEIVER_SOURCE_ID.into()),
                CaptureBackend::Direct,
            ) {
            Ok(snapshot) => {
                self.record_session_start_success(
                    device_id,
                    snapshot.workspace.selected_source_id.as_deref(),
                );
                self.apply_runtime_snapshot(snapshot);
            }
            Err(error) => {
                let message = error.to_string();
                self.session = SessionViewModel::error(&message);
                self.diagnostics.host_error = Some(message.clone());
                self.diagnostics.control_summary = "control blocked".into();
                self.diagnostics.grounding_summary = "grounding blocked".into();
                self.record_session_start_failure(Some(device_id), &message);
            }
        }
    }

    pub fn can_start_direct_receiver(&self) -> bool {
        self.host_runtime.is_some()
            && self.startup.direct_receiver.available
            && !self
                .runtime_statuses
                .iter()
                .any(|status| status.summary().device_id == Self::DIRECT_RECEIVER_DEVICE_ID)
    }

    pub fn request_start_direct_receiver(&mut self) {
        if self
            .runtime_statuses
            .iter()
            .any(|status| status.summary().device_id == Self::DIRECT_RECEIVER_DEVICE_ID)
        {
            return;
        }

        let device_id = Self::DIRECT_RECEIVER_DEVICE_ID.to_string();
        self.selected_device_id = Some(device_id.clone());
        self.next_runtime_refresh_at = None;
        self.runtime_refresh_device_id = None;

        if self.host_runtime.is_none() {
            let message = "Host runtime unavailable";
            self.session = SessionViewModel::error(message);
            self.diagnostics.host_error = Some(message.into());
            self.diagnostics.control_summary = "control blocked".into();
            self.diagnostics.grounding_summary = "grounding blocked".into();
            self.record_session_start_failure(Some(&device_id), message);
            return;
        }

        if !self.startup.direct_receiver.available {
            let message = self.startup.direct_receiver.detail.clone();
            self.session = SessionViewModel::error(&message);
            self.diagnostics.host_error = Some(message.clone());
            self.diagnostics.control_summary = "control blocked".into();
            self.diagnostics.grounding_summary = "grounding blocked".into();
            self.record_session_start_failure(Some(&device_id), &message);
            return;
        }

        self.session = SessionViewModel::starting();
        self.session_window_auto_sized_frame = None;
        self.diagnostics.host_error = None;
        self.diagnostics.control_summary = "control bootstrapping".into();
        self.diagnostics.grounding_summary = "grounding bootstrapping".into();
        self.record_session_start_attempt(&device_id, Some(Self::DIRECT_RECEIVER_SOURCE_ID));

        match self
            .host_runtime
            .as_mut()
            .expect("host runtime should be present")
            .start_session(
                &device_id,
                Self::DIRECT_RECEIVER_DEVICE_NAME,
                Some(Self::DIRECT_RECEIVER_SOURCE_ID.into()),
                CaptureBackend::Direct,
            ) {
            Ok(snapshot) => {
                self.record_session_start_success(
                    &device_id,
                    snapshot.workspace.selected_source_id.as_deref(),
                );
                self.apply_runtime_snapshot(snapshot);
                self.refresh_inventory();
            }
            Err(error) => {
                let message = error.to_string();
                self.session = SessionViewModel::error(&message);
                self.diagnostics.host_error = Some(message.clone());
                self.diagnostics.control_summary = "control blocked".into();
                self.diagnostics.grounding_summary = "grounding blocked".into();
                self.record_session_start_failure(Some(&device_id), &message);
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
        self.next_inventory_refresh_at = None;
        self.next_runtime_refresh_at = None;
        self.runtime_refresh_device_id = None;
        self.preview_texture = None;
        self.preview_content_bounds = None;
        self.preview_input_bridge.reset();
        self.restored_source_preference = None;
        self.manual_source_selection_device_id = None;
        self.session_window_open = false;
        self.session_window_deferred_until_streaming = false;
        self.session_window_focus_requested = false;
        self.session_window_device_id = None;
        self.session_window_auto_sized_frame = None;
        self.session = SessionViewModel::idle();
        self.diagnostics.host_error = None;
        self.diagnostics.control_summary = "control not started".into();
        self.diagnostics.grounding_summary = "grounding idle".into();
        self.sync_from_inventory_and_runtime();
    }

    pub fn select_capture_source(&mut self, source_id: &str) {
        let Some(source) = self.device_detail.capture_source(source_id) else {
            return;
        };

        self.device_detail.active_source_id = Some(source.source_id.clone());
        self.session.selected_source = Some(source.clone());
        self.session.start_enabled = !matches!(
            self.session.ui_state,
            crate::view_models::session::SessionUiState::Streaming
                | crate::view_models::session::SessionUiState::Starting
                | crate::view_models::session::SessionUiState::WaitingForMirror
        );
        self.manual_source_selection_device_id = self.selected_device_id.clone();
        if let Some(device_id) = self.selected_device_id.clone() {
            self.clear_restored_source_preference_for_device(device_id.as_str());
        }
        self.preferences.selected_device_id = self.selected_device_id.clone();
        self.preferences.selected_source_id = Some(source.source_id);
        self.persist_preferences();
    }

    pub fn apply_runtime_snapshot(&mut self, snapshot: HostRuntimeSnapshot) {
        self.selected_device_id = Some(snapshot.workspace.device_id.clone());
        self.runtime_statuses = snapshot.statuses.clone();
        self.runtime_workspace = Some(snapshot.workspace.clone());
        self.sync_from_inventory_and_runtime();
    }

    fn forward_preview_input(
        &mut self,
        events: Vec<ios_control_contracts::control::ControlInputEvent>,
    ) {
        let preview_input_log = std::env::var_os("IOS_CONTROL_PREVIEW_INPUT_LOG").is_some();
        if preview_input_log {
            self.append_host_log_line(&format!(
                "preview input forwarding {} event(s): {}",
                events.len(),
                preview_input_event_summary(&events)
            ));
        }
        let Some(device_id) = self.selected_device_id.clone() else {
            return;
        };
        if self.host_runtime.is_none() {
            self.diagnostics.host_error =
                Some("Preview input failed: host runtime unavailable".into());
            return;
        }

        for event in events {
            let result = self
                .host_runtime
                .as_mut()
                .expect("host runtime checked above")
                .forward_control_input(&device_id, event);
            match result {
                Ok(summary) => {
                    if preview_input_log {
                        self.append_host_log_line(&format!(
                            "preview input result: {} {:?}",
                            summary.summary, summary.phase
                        ));
                    }
                    self.diagnostics.control_summary = summary.summary;
                }
                Err(error) => {
                    self.diagnostics.host_error =
                        Some(format!("Preview input failed for {device_id}: {error}"));
                    break;
                }
            }
        }
    }

    fn sync_from_inventory_and_runtime(&mut self) {
        let statuses = &self.runtime_statuses;
        self.fleet = FleetViewModel::from_inventory(&self.inventory_snapshot.devices, statuses);
        self.available_device_ids = self
            .fleet
            .rows
            .iter()
            .map(|row| row.device_id.clone())
            .collect();
        let degraded_devices = self
            .fleet
            .rows
            .iter()
            .filter(|row| row.operator_action.is_some() || row.readiness_summary == "Not startable")
            .count();
        self.dashboard =
            DashboardViewModel::from_inventory_rows(self.fleet.rows.len(), degraded_devices);

        if self.selected_device_id.as_deref().is_none_or(|selected| {
            !self.available_device_ids.iter().any(|id| id == selected)
                && !self
                    .runtime_statuses
                    .iter()
                    .any(|status| status.summary().device_id == selected)
        }) {
            self.selected_device_id = self.available_device_ids.first().cloned();
        }

        self.sync_selected_workspace();
        self.reconcile_session_window_state();
        self.try_start_pending_device();
    }

    fn try_start_pending_device(&mut self) {
        let Some(device_id) = self.pending_start_device_id.clone() else {
            return;
        };
        if self
            .runtime_statuses
            .iter()
            .any(|status| status.summary().device_id == device_id)
        {
            self.pending_start_device_id = None;
            return;
        }

        if device_id == Self::DIRECT_RECEIVER_DEVICE_ID {
            if self.can_start_direct_receiver() {
                self.pending_start_device_id = None;
                self.session_window_open = true;
                self.session_window_deferred_until_streaming = false;
                self.session_window_focus_requested = true;
                self.session_window_auto_sized_frame = None;
                self.request_start_direct_receiver();
            }
            return;
        }

        let launcher = FleetViewModel::for_launcher(
            &self.inventory_snapshot.devices,
            self.startup.direct_receiver.available,
            &self.runtime_statuses,
        );
        let startable = launcher
            .rows
            .iter()
            .find(|row| row.device_id == device_id)
            .is_some_and(|row| row.start_enabled);
        if startable {
            self.pending_start_device_id = None;
            self.select_device(&device_id);
            self.request_open_selected_device_session();
        }
    }

    fn sync_selected_workspace(&mut self) {
        let Some(selected_device_id) = self.selected_device_id.clone() else {
            self.device_detail.device_name = "No device selected".into();
            self.device_detail.capture_sources.clear();
            self.device_detail.active_source_id = None;
            self.device_detail.control_checklist = ControlSetupChecklist { items: Vec::new() };
            self.device_detail.inventory_notes.clear();
            self.session = SessionViewModel::idle();
            self.reconcile_session_window_state();
            return;
        };
        let status = self
            .runtime_statuses
            .iter()
            .find(|status| status.summary().device_id == selected_device_id)
            .cloned();

        if let Some(workspace) = self
            .runtime_workspace
            .as_ref()
            .filter(|workspace| workspace.device_id == selected_device_id)
            .cloned()
        {
            let status = status.expect("workspace should have matching runtime status");
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
            self.device_detail.inventory_notes.clear();

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
            let capture_detail = workspace
                .capture_status
                .as_ref()
                .and_then(|status| status.detail.clone());

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
                | SessionSubstate::Recovering => {
                    if status.backends().capture_backend == "capture.direct"
                        && workspace.latest_frame.is_none()
                    {
                        SessionViewModel::waiting_for_mirror(Some(source))
                    } else {
                        SessionViewModel::starting()
                    }
                }
                SessionSubstate::OperatorActionRequired
                | SessionSubstate::DegradedCapture
                | SessionSubstate::DegradedControl => SessionViewModel::error(
                    status
                        .operator_action()
                        .unwrap_or("Session requires operator intervention"),
                ),
                SessionSubstate::Stopped => SessionViewModel::idle(),
            }
            .with_status_detail(capture_detail);
            self.reconcile_session_window_state();
            return;
        }

        if let Some(status) = status {
            self.device_detail.device_name = status.summary().device_name.clone();
            let source = capture_source_for_backend(status.backends().capture_backend.as_str());
            self.device_detail.capture_sources = vec![source.clone()];
            self.device_detail.active_source_id = Some(source.source_id.clone());
            self.device_detail.control_checklist = ControlSetupChecklist::for_pointer_mode();
            self.device_detail.inventory_notes.clear();

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
                | SessionSubstate::Recovering => {
                    if status.backends().capture_backend == "capture.direct" {
                        SessionViewModel::waiting_for_mirror(Some(source))
                    } else {
                        SessionViewModel::starting()
                    }
                }
                SessionSubstate::OperatorActionRequired
                | SessionSubstate::DegradedCapture
                | SessionSubstate::DegradedControl => SessionViewModel::error(
                    status
                        .operator_action()
                        .unwrap_or("Session requires operator intervention"),
                ),
                SessionSubstate::Stopped => SessionViewModel::idle(),
            };
            self.reconcile_session_window_state();
            return;
        }

        let Some(device) = self
            .inventory_snapshot
            .devices
            .iter()
            .find(|device| device.inventory_id == selected_device_id)
            .cloned()
        else {
            return;
        };

        self.apply_inventory_detail(device);
        self.reconcile_session_window_state();
    }

    fn reconcile_session_window_state(&mut self) {
        if !self.session_window_deferred_until_streaming {
            return;
        }

        match self.session.ui_state {
            crate::view_models::session::SessionUiState::WaitingForMirror
            | crate::view_models::session::SessionUiState::Starting => {}
            crate::view_models::session::SessionUiState::Streaming
            | crate::view_models::session::SessionUiState::Blocked(_)
            | crate::view_models::session::SessionUiState::Error(_) => {
                self.session_window_open = true;
                self.session_window_deferred_until_streaming = false;
                self.session_window_focus_requested = true;
                self.session_window_auto_sized_frame = None;
            }
            crate::view_models::session::SessionUiState::Idle => {
                self.session_window_deferred_until_streaming = false;
                self.session_window_focus_requested = false;
            }
        }
    }

    fn apply_inventory_detail(&mut self, device: InventoryDevice) {
        let capture_sources = self.capture_sources_for_device(&device);
        let selected_source = device
            .mirror_source_id
            .as_ref()
            .and_then(|source_id| {
                capture_sources
                    .iter()
                    .find(|source| source.source_id == *source_id)
            })
            .cloned()
            .or_else(|| {
                self.device_detail
                    .active_source_id
                    .as_ref()
                    .and_then(|source_id| {
                        capture_sources
                            .iter()
                            .find(|source| source.source_id == *source_id)
                    })
                    .cloned()
            });
        self.device_detail.device_name = device.display_name.clone();
        self.device_detail.capture_sources = capture_sources;
        self.device_detail.active_source_id = selected_source
            .as_ref()
            .map(|source| source.source_id.clone());
        self.device_detail.control_checklist = ControlSetupChecklist {
            items: device.reasons.clone(),
        };
        self.device_detail.inventory_notes = inventory_notes(&device);

        self.diagnostics.host_error = None;
        self.diagnostics.control_summary = match device.sessionability {
            Sessionability::StartableWithPreferredPath => "preferred control ready".into(),
            Sessionability::StartableWithFallback => "fallback control ready".into(),
            Sessionability::NotStartable => "control path incomplete".into(),
            Sessionability::Unknown => "known device only".into(),
        };
        self.diagnostics.grounding_summary = if device.live {
            "inventory discovered device".into()
        } else {
            "historical inventory device".into()
        };

        self.session = match device.sessionability {
            Sessionability::StartableWithPreferredPath | Sessionability::StartableWithFallback => {
                SessionViewModel::idle_startable(selected_source)
            }
            Sessionability::NotStartable => SessionViewModel::blocked(
                first_reason_or_default(&device, "Device is not startable yet"),
                selected_source,
            ),
            Sessionability::Unknown => SessionViewModel::blocked(
                first_reason_or_default(&device, "Known device is waiting for live evidence"),
                selected_source,
            ),
        };
    }

    fn capture_sources_for_device(&self, device: &InventoryDevice) -> Vec<CaptureSourceOption> {
        if let Some(source_id) = device.mirror_source_id.as_ref() {
            return vec![self.capture_source_option(source_id, &device.display_name)];
        }

        let mut seen = std::collections::BTreeSet::new();
        self.inventory_snapshot
            .devices
            .iter()
            .filter(|candidate| candidate.live)
            .filter_map(|candidate| {
                candidate
                    .mirror_source_id
                    .as_ref()
                    .filter(|source_id| seen.insert((**source_id).to_string()))
                    .map(|source_id| self.capture_source_option(source_id, &candidate.display_name))
            })
            .collect()
    }

    fn capture_source_option(&self, source_id: &str, fallback_name: &str) -> CaptureSourceOption {
        let display_name = self
            .inventory_snapshot
            .devices
            .iter()
            .find(|candidate| candidate.mirror_source_id.as_deref() == Some(source_id))
            .map(|candidate| candidate.display_name.as_str())
            .unwrap_or(fallback_name);
        CaptureSourceOption::new(source_id, display_name)
    }

    fn poll_inventory_refresh_if_due(&mut self, now: Instant) {
        if self.runtime_config.is_none() {
            self.next_inventory_refresh_at = None;
            return;
        }
        if self
            .next_inventory_refresh_at
            .is_some_and(|next| now < next)
        {
            return;
        }
        self.next_inventory_refresh_at = Some(now + Self::INVENTORY_REFRESH_POLL_INTERVAL);
        self.refresh_inventory();
    }

    fn sync_preview_texture(&mut self, ctx: &egui::Context) {
        let Some(workspace) = self.runtime_workspace.as_ref() else {
            self.preview_texture = None;
            self.preview_content_bounds = None;
            return;
        };
        let Some(stream) = workspace.capture_stream.as_ref() else {
            self.preview_texture = None;
            self.preview_content_bounds = None;
            return;
        };
        let Some(frame) = workspace.latest_frame.as_ref() else {
            self.preview_texture = None;
            self.preview_content_bounds = None;
            return;
        };
        let Ok(image) = color_image_from_slot(stream, frame) else {
            self.preview_texture = None;
            self.preview_content_bounds = None;
            return;
        };
        self.preview_content_bounds = detect_visible_content_bounds(&image);

        if let Some(texture) = self.preview_texture.as_mut() {
            texture.set(image, egui::TextureOptions::LINEAR);
        } else {
            self.preview_texture =
                Some(ctx.load_texture("session-preview", image, egui::TextureOptions::LINEAR));
        }
    }

    fn auto_size_session_window_to_frame(&mut self, ctx: &egui::Context) -> bool {
        let Some(frame) = self.session.latest_frame.as_ref() else {
            return false;
        };
        let frame_size = [frame.width.max(1), frame.height.max(1)];
        let content_bounds = self
            .preview_content_bounds
            .map(|bounds| bounds.normalized_for_frame(frame_size));
        let frame_key = (
            frame.width,
            frame.height,
            frame.rotation_degrees,
            content_bounds,
        );
        if self.session_window_auto_sized_frame == Some(frame_key) {
            return false;
        }

        let monitor_size = ctx
            .input(|input| input.viewport().monitor_size)
            .unwrap_or_else(|| egui::vec2(900.0, 700.0));
        let inner_size = session_view::auto_session_inner_size(
            frame,
            content_bounds,
            monitor_size,
            ctx.pixels_per_point(),
        );
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(inner_size));
        self.session_window_auto_sized_frame = Some(frame_key);
        self.session_window_resize_pending = true;
        true
    }

    fn correct_session_window_aspect_to_frame(&mut self, ctx: &egui::Context) {
        if self.session_window_resize_pending {
            self.session_window_resize_pending = false;
            ctx.request_repaint();
            return;
        }
        let Some(frame) = self.session.latest_frame.as_ref() else {
            return;
        };
        let Some(current_size) =
            ctx.input(|input| input.viewport().inner_rect.map(|rect| rect.size()))
        else {
            return;
        };
        let frame_size = [frame.width.max(1), frame.height.max(1)];
        let content_bounds = self
            .preview_content_bounds
            .map(|bounds| bounds.normalized_for_frame(frame_size));
        let target_size = session_view::aspect_corrected_session_inner_size(
            frame,
            content_bounds,
            current_size,
            ctx.pixels_per_point(),
        );
        if session_view::session_inner_size_needs_correction(current_size, target_size) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target_size));
        }
    }

    fn selected_runtime_refresh_target(&self) -> Option<(&str, Duration)> {
        let device_id = self.selected_runtime_streaming_device_id()?;
        let waiting_for_direct_frame = self
            .runtime_statuses
            .iter()
            .find(|status| status.summary().device_id == device_id)
            .is_some_and(|status| status.backends().capture_backend == "capture.direct")
            && self
                .runtime_workspace
                .as_ref()
                .filter(|workspace| workspace.device_id == device_id)
                .is_some_and(|workspace| workspace.latest_frame.is_none());
        let interval = if waiting_for_direct_frame {
            Self::WAITING_FOR_MIRROR_REFRESH_POLL_INTERVAL
        } else {
            Self::RUNTIME_REFRESH_POLL_INTERVAL
        };
        Some((device_id, interval))
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
                ) || (status.backends().capture_backend == "capture.direct"
                    && self
                        .runtime_workspace
                        .as_ref()
                        .filter(|workspace| workspace.device_id == device_id)
                        .is_some_and(|workspace| workspace.latest_frame.is_none()))
            });
        if streaming {
            Some(device_id)
        } else {
            None
        }
    }

    fn poll_runtime_refresh_if_due(&mut self, now: Instant) {
        let Some((device_id, refresh_interval)) = self
            .selected_runtime_refresh_target()
            .map(|(device_id, interval)| (device_id.to_string(), interval))
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
        self.next_runtime_refresh_at = Some(now + refresh_interval);

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

fn inventory_notes(device: &InventoryDevice) -> Vec<String> {
    let mut notes = device
        .evidence_sources
        .iter()
        .map(|source| match source {
            crate::inventory::model::InventoryEvidenceSource::Bluetooth => "paired over bluetooth",
            crate::inventory::model::InventoryEvidenceSource::Mirror => {
                "live mirror source observed"
            }
            crate::inventory::model::InventoryEvidenceSource::Known => "known from history only",
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    notes.extend(device.reasons.clone());
    notes
}

fn is_transient_bluetooth_row(device: &InventoryDevice) -> bool {
    device.live
        && device
            .evidence_sources
            .contains(&InventoryEvidenceSource::Bluetooth)
}

fn first_reason_or_default(device: &InventoryDevice, default: &str) -> String {
    if let Some(reason) = device
        .reasons
        .iter()
        .find(|reason| {
            let lowered = reason.to_ascii_lowercase();
            lowered.contains("no capture")
                || lowered.contains("unavailable")
                || lowered.contains("missing")
        })
        .cloned()
    {
        return sentence_case(reason);
    }
    sentence_case(
        device
            .reasons
            .first()
            .cloned()
            .unwrap_or_else(|| default.to_string()),
    )
}

fn sentence_case(message: String) -> String {
    let mut chars = message.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => message,
    }
}

impl eframe::App for HostDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut pending_action = SessionAction::None;
        let mut launcher_action = LauncherAction::None;
        let mut settings_action = settings::SettingsAction::None;

        if let Some((_, interval)) = self.selected_runtime_refresh_target() {
            ctx.request_repaint_after(interval);
        }
        let now = Instant::now();
        self.poll_inventory_refresh_if_due(now);
        self.poll_runtime_refresh_if_due(now);

        self.sync_preview_texture(ctx);

        let launcher_rows = FleetViewModel::for_launcher(
            &self.inventory_snapshot.devices,
            self.startup.direct_receiver.available,
            &self.runtime_statuses,
        );
        let session_menu_device_id = self
            .runtime_workspace
            .as_ref()
            .map(|workspace| workspace.device_id.clone())
            .or_else(|| self.session_window_device_id.clone())
            .or_else(|| self.selected_device_id.clone());
        egui::CentralPanel::default().show(ctx, |ui| {
            let (device_action, control_host_session_action) = launcher::render_with_session_menu(
                ui,
                &launcher_rows,
                self.selected_device_id.as_deref(),
                session_menu_device_id.as_deref(),
                &self.session,
            );
            launcher_action = device_action;
            if !matches!(control_host_session_action, SessionAction::None) {
                pending_action = control_host_session_action;
            }
            ui.separator();
            settings_action = settings::render_rows(
                ui,
                &self.settings.rows,
                self.preview_input_bridge.pointer_long_axis_units(),
                self.preferences.direct_preview_fps,
                self.preferences.direct_preview_height,
            );
        });

        if self.session_window_open {
            let viewport_id = egui::ViewportId::from_hash_of("host-desktop-session-window");
            let title = if self.device_detail.device_name.is_empty() {
                "Device Session".to_string()
            } else {
                format!("Session | {}", self.device_detail.device_name)
            };
            ctx.show_viewport_immediate(
                viewport_id,
                egui::ViewportBuilder::default()
                    .with_title(title)
                    .with_inner_size([900.0, 700.0])
                    .with_clamp_size_to_monitor_size(true)
                    .with_decorations(true)
                    .with_active(true),
                |ctx, _class| {
                    if ctx.input(|input| input.viewport().close_requested()) {
                        self.session_window_open = false;
                        self.session_window_deferred_until_streaming = false;
                        self.session_window_focus_requested = false;
                        self.session_window_device_id = None;
                        self.session_window_auto_sized_frame = None;
                        return;
                    }
                    if self.session_window_focus_requested {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        self.session_window_focus_requested = false;
                    }
                    if !self.auto_size_session_window_to_frame(ctx) {
                        self.correct_session_window_aspect_to_frame(ctx);
                    }

                    egui::CentralPanel::default()
                        .frame(
                            egui::Frame::default()
                                .fill(egui::Color32::from_rgb(242, 242, 247))
                                .inner_margin(0.0),
                        )
                        .show(ctx, |ui| {
                            let session_window_action = session_view::render(
                                ui,
                                &self.session,
                                self.preview_texture.as_ref(),
                                self.preview_content_bounds,
                                &mut self.preview_input_bridge,
                            );
                            if !matches!(session_window_action, SessionAction::None) {
                                pending_action = session_window_action;
                            }
                        });
                },
            );
        }

        match launcher_action {
            LauncherAction::None => {}
            LauncherAction::SelectDevice(device_id) => {
                self.select_device(&device_id);
                ctx.request_repaint();
            }
            LauncherAction::OpenDevice(device_id) => {
                self.select_device(&device_id);
                self.request_open_selected_device_session();
                ctx.request_repaint();
            }
        }

        match settings_action {
            settings::SettingsAction::None => {}
            settings::SettingsAction::SetBlePointerLongAxisUnits(units) => {
                self.set_ble_pointer_long_axis_units(units);
                ctx.request_repaint();
            }
            settings::SettingsAction::SetDirectPreviewFps(fps) => {
                self.set_direct_preview_fps(fps);
                ctx.request_repaint();
            }
            settings::SettingsAction::SetDirectPreviewHeight(height) => {
                self.set_direct_preview_height(height);
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
            SessionAction::ControlInput(events) => {
                self.forward_preview_input(events);
                ctx.request_repaint();
            }
        }
    }
}

fn preview_input_event_summary(
    events: &[ios_control_contracts::control::ControlInputEvent],
) -> String {
    events
        .iter()
        .map(|event| match event {
            ios_control_contracts::control::ControlInputEvent::Mouse(mouse) => format!(
                "mouse buttons={} dx={} dy={} wheel={}",
                mouse.buttons, mouse.dx, mouse.dy, mouse.wheel
            ),
            ios_control_contracts::control::ControlInputEvent::MouseSequence(reports) => {
                let preview = reports
                    .iter()
                    .take(6)
                    .map(|mouse| {
                        format!(
                            "buttons={} dx={} dy={} wheel={}",
                            mouse.buttons, mouse.dx, mouse.dy, mouse.wheel
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let suffix = if reports.len() > 6 { ", ..." } else { "" };
                format!(
                    "mouse-sequence reports={} [{}{}]",
                    reports.len(),
                    preview,
                    suffix
                )
            }
            ios_control_contracts::control::ControlInputEvent::Keyboard(keyboard) => format!(
                "keyboard usage={} pressed={} shift={} alt={} ctrl={} meta={}",
                keyboard.usage_id,
                keyboard.pressed,
                keyboard.modifiers.shift,
                keyboard.modifiers.alt,
                keyboard.modifiers.ctrl,
                keyboard.modifiers.meta
            ),
            ios_control_contracts::control::ControlInputEvent::Text(text) => {
                format!("text chars={}", text.chars().count())
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Color32, ColorImage};
    use ios_control_contracts::control::ControlSessionPhase;
    use ios_control_contracts::plugin::PluginHealth;
    use ios_control_contracts::session::{
        BackendSelection, DeviceSessionStatus, DeviceSessionSummary, SessionPhase,
    };
    use ios_control_session_orchestrator::{PluginPaths, SessionDiagnostics};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn app_with_runtime_without_active_session() -> HostDesktopApp {
        HostDesktopApp::with_runtime(HostRuntimeConfig {
            plugin_paths: PluginPaths {
                capture: PathBuf::from("missing-capture-plugin"),
                capture_direct: PathBuf::from("missing-direct-capture-plugin"),
                capture_direct_runtime_root: None,
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
                capture_status: None,
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

    fn direct_waiting_snapshot_without_frame() -> HostRuntimeSnapshot {
        let status = DeviceSessionStatus::new(
            DeviceSessionSummary {
                device_id: "direct-receiver".into(),
                device_name: "Direct Receiver".into(),
                phase: SessionPhase::Connecting,
                plugin_health: PluginHealth::Healthy,
                capture_plugin: Some("capture.direct".into()),
                control_plugin: Some("control.ble".into()),
                grounding_plugin: None,
            },
            SessionSubstate::StartingCapture,
            BackendSelection {
                capture_backend: "capture.direct".into(),
                control_backend: "control.ble".into(),
            },
            None,
        )
        .expect("valid direct waiting status");

        HostRuntimeSnapshot {
            statuses: vec![status.clone()],
            workspace: RuntimeWorkspaceState {
                device_id: "direct-receiver".into(),
                summary: status.summary().clone(),
                capture_sources: vec![ios_control_contracts::capture::VideoSource {
                    source_id: "direct-1".into(),
                    display_name: "Direct Receiver".into(),
                    kind: ios_control_contracts::capture::SourceKind::DirectReceiver,
                }],
                capture_stream: None,
                capture_status: None,
                latest_frame: None,
                selected_source_id: Some("direct-1".into()),
                control_checklist: ios_control_contracts::control::ControlSetupChecklist {
                    items: vec!["Pair the device".into()],
                },
                control_phase: ControlSessionPhase::Connected,
                execution_observed_change: Some(true),
                diagnostics: SessionDiagnostics {
                    control_phase: ControlSessionPhase::Connected,
                    control_summary: "control ready".into(),
                    grounding_summary: Some("waiting for mirror".into()),
                },
            },
        }
    }

    fn bluetooth_inventory_snapshot() -> InventorySnapshot {
        crate::inventory::aggregator::aggregate_inventory(vec![
            crate::inventory::model::DeviceObservation {
                provider: InventoryEvidenceSource::Bluetooth,
                stable_id: Some("bt:AA-BB".into()),
                known_device_id: None,
                display_name: "Alice iPhone".into(),
                mirror_source_id: None,
                live: true,
                capture_state: crate::inventory::model::CapabilityState::Unavailable,
                preferred_control_state: crate::inventory::model::CapabilityState::Ready,
                fallback_control_state: crate::inventory::model::CapabilityState::Unavailable,
                reasons: vec!["paired over bluetooth".into()],
            },
        ])
    }

    #[test]
    fn transient_bluetooth_inventory_rows_age_out_after_repeated_misses() {
        let mut app = HostDesktopApp::new();
        app.apply_inventory_snapshot(bluetooth_inventory_snapshot());

        for _ in 0..HostDesktopApp::INVENTORY_MISSED_REFRESH_GRACE {
            app.apply_inventory_snapshot(InventorySnapshot::default());
            assert_eq!(app.available_device_ids, vec!["bt:AA-BB"]);
        }

        app.apply_inventory_snapshot(InventorySnapshot::default());

        assert!(app.available_device_ids.is_empty());
        assert!(app.fleet.rows.is_empty());
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
    fn direct_waiting_for_first_frame_uses_slower_refresh_interval() {
        let mut app = app_with_runtime_without_active_session();
        app.apply_runtime_snapshot(direct_waiting_snapshot_without_frame());

        let start = Instant::now();
        app.poll_runtime_refresh_if_due(start);
        assert!(app
            .diagnostics
            .host_error
            .as_deref()
            .is_some_and(|error| error.contains("Runtime refresh failed")));

        app.diagnostics.host_error = None;
        app.poll_runtime_refresh_if_due(start + HostDesktopApp::RUNTIME_REFRESH_POLL_INTERVAL * 2);
        assert_eq!(app.diagnostics.host_error, None);

        app.poll_runtime_refresh_if_due(
            start
                + HostDesktopApp::WAITING_FOR_MIRROR_REFRESH_POLL_INTERVAL
                + Duration::from_millis(1),
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
