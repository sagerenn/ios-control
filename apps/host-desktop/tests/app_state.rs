use host_desktop::app::HostDesktopApp;
use host_desktop::inventory::aggregator::aggregate_inventory;
use host_desktop::inventory::model::{
    CapabilityState, DeviceObservation, InventoryEvidenceSource,
};
use host_desktop::panels::device_detail::{CaptureSourceOption, ControlSetupChecklist};
use host_desktop::preferences::HostPreferencesStore;
use host_desktop::runtime::{HostRuntimeConfig, HostRuntimeSnapshot, RuntimeWorkspaceState};
use host_desktop::view_models::session::SessionUiState;
use ios_control_contracts::control::ControlSessionPhase;
use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{
    BackendSelection, DeviceSessionStatus, DeviceSessionSummary, SessionPhase, SessionSubstate,
};
use ios_control_session_orchestrator::{PluginPaths, SessionDiagnostics};
use std::path::PathBuf;

mod support;
use support::{
    build_plugins, host_plugin_paths, prepare_window_runtime_env, runtime_env_lock, workspace_root,
    EnvVarGuards,
};

struct RuntimeAppFixture {
    _lock: std::sync::MutexGuard<'static, ()>,
    _guards: EnvVarGuards,
    preferences_path: Option<PathBuf>,
    app: HostDesktopApp,
}

fn host_app_with_runtime() -> RuntimeAppFixture {
    let lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let guards = prepare_window_runtime_env(&root);
    let app = HostDesktopApp::with_runtime(HostRuntimeConfig {
        plugin_paths: host_plugin_paths(&root),
    });

    RuntimeAppFixture {
        _lock: lock,
        _guards: guards,
        preferences_path: None,
        app,
    }
}

fn first_discovered_device_id(app: &HostDesktopApp) -> String {
    app.available_device_ids
        .first()
        .cloned()
        .expect("at least one discovered device should exist")
}

fn host_app_with_runtime_and_preferences(preferences_json: &str) -> RuntimeAppFixture {
    let lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let guards = prepare_window_runtime_env(&root);
    let prefs_path = support::write_preferences_json(preferences_json);
    let app = HostDesktopApp::with_runtime_and_preferences(
        HostRuntimeConfig {
            plugin_paths: host_plugin_paths(&root),
        },
        host_desktop::preferences::HostPreferencesStore::new(prefs_path.clone()),
    );

    RuntimeAppFixture {
        _lock: lock,
        _guards: guards,
        preferences_path: Some(prefs_path),
        app,
    }
}

fn host_app_with_missing_runtime_plugins_and_preferences(
    preferences_json: &str,
) -> RuntimeAppFixture {
    let lock = runtime_env_lock();
    let prefs_path = support::write_preferences_json(preferences_json);
    let app = HostDesktopApp::with_runtime_and_preferences(
        HostRuntimeConfig {
            plugin_paths: PluginPaths {
                capture: PathBuf::from("missing-capture-plugin"),
                control_ble: PathBuf::from("missing-control-ble-plugin"),
                control_fallback: PathBuf::from("missing-control-fallback-plugin"),
                grounding: None,
            },
        },
        host_desktop::preferences::HostPreferencesStore::new(prefs_path.clone()),
    );

    RuntimeAppFixture {
        _lock: lock,
        _guards: EnvVarGuards::new(vec![]),
        preferences_path: Some(prefs_path),
        app,
    }
}

fn runtime_snapshot_with_control(
    control_phase: ControlSessionPhase,
    control_summary: &str,
    execution_observed_change: Option<bool>,
) -> HostRuntimeSnapshot {
    let status = status(
        "device-1",
        "Alpha",
        SessionPhase::Degraded,
        SessionSubstate::OperatorActionRequired,
        "capture.window.helper",
        "control.ble",
        Some("Reconnect BLE helper"),
    );

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
            control_phase,
            execution_observed_change,
            diagnostics: SessionDiagnostics {
                control_phase,
                control_summary: control_summary.into(),
                grounding_summary: Some("selected pointer plan".into()),
            },
        },
    }
}

fn runtime_snapshot_with_frame(
    frame: ios_control_contracts::capture::VideoFrameDescriptor,
) -> HostRuntimeSnapshot {
    let status = status(
        "device-1",
        "Alpha",
        SessionPhase::Streaming,
        SessionSubstate::Streaming,
        "capture.window.helper",
        "control.ble",
        None,
    );

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
            latest_frame: Some(frame),
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

fn runtime_snapshot_streaming_without_frame() -> HostRuntimeSnapshot {
    let status = status(
        "device-1",
        "Alpha",
        SessionPhase::Streaming,
        SessionSubstate::Streaming,
        "capture.window.helper",
        "control.ble",
        None,
    );

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

fn host_app_from_runtime_snapshot(snapshot: HostRuntimeSnapshot) -> HostDesktopApp {
    let mut app = HostDesktopApp::new();
    app.apply_runtime_snapshot(snapshot);
    app
}

fn partially_discovered_inventory() -> host_desktop::inventory::model::InventorySnapshot {
    aggregate_inventory(vec![DeviceObservation {
        provider: InventoryEvidenceSource::Bluetooth,
        stable_id: Some("bt:AA-BB".into()),
        known_device_id: None,
        display_name: "Alice iPhone".into(),
        mirror_source_id: None,
        live: true,
        capture_state: CapabilityState::Unavailable,
        preferred_control_state: CapabilityState::Discovered,
        fallback_control_state: CapabilityState::Unavailable,
        reasons: vec!["paired over bluetooth".into(), "no capture path observed".into()],
    }])
}

fn bluetooth_and_unlinked_mirror_inventory() -> host_desktop::inventory::model::InventorySnapshot {
    aggregate_inventory(vec![
        DeviceObservation {
            provider: InventoryEvidenceSource::Bluetooth,
            stable_id: Some("bt:AA-BB".into()),
            known_device_id: None,
            display_name: "Alice iPhone".into(),
            mirror_source_id: None,
            live: true,
            capture_state: CapabilityState::Unavailable,
            preferred_control_state: CapabilityState::Ready,
            fallback_control_state: CapabilityState::Unavailable,
            reasons: vec!["paired over bluetooth".into(), "no capture path observed".into()],
        },
        DeviceObservation {
            provider: InventoryEvidenceSource::Mirror,
            stable_id: None,
            known_device_id: None,
            display_name: "Operator Mirror".into(),
            mirror_source_id: Some("window-helper-1".into()),
            live: true,
            capture_state: CapabilityState::Ready,
            preferred_control_state: CapabilityState::Unavailable,
            fallback_control_state: CapabilityState::Ready,
            reasons: vec![],
        },
    ])
}

#[test]
fn host_app_boots_without_inventing_a_mock_device() {
    let app = HostDesktopApp::new();

    assert_eq!(app.dashboard.total_devices, 0);
    assert_eq!(app.dashboard.degraded_devices, 0);
    assert!(app.available_device_ids.is_empty());
    assert!(app.selected_device_id.is_none());
    assert_eq!(app.device_detail.device_name, "No device selected");
    assert!(app.device_detail.capture_sources.is_empty());
    assert_eq!(app.device_detail.control_checklist.items, Vec::<String>::new());
    assert_eq!(app.session.ui_state, SessionUiState::Idle);
    assert!(app.session.selected_source.is_none());
    assert!(app.session.latest_frame.is_none());
    assert_eq!(app.diagnostics.host_error, None);
    assert_eq!(app.diagnostics.control_summary, "control not started");
    assert_eq!(app.diagnostics.grounding_summary, "grounding idle");
    assert_eq!(app.startup.readiness, host_desktop::view_models::startup::StartupReadiness::Blocked);
    assert!(app.startup.summary.contains("Blocked"));
}

#[test]
fn host_app_with_missing_runtime_components_starts_blocked_without_fake_device() {
    let fixture = host_app_with_missing_runtime_plugins_and_preferences("{}");

    assert_eq!(fixture.app.dashboard.total_devices, 0);
    assert!(fixture.app.available_device_ids.is_empty());
    assert!(fixture.app.selected_device_id.is_none());
    assert_eq!(fixture.app.session.ui_state, SessionUiState::Idle);
    assert_eq!(
        fixture.app.startup.readiness,
        host_desktop::view_models::startup::StartupReadiness::Blocked
    );
    assert!(fixture
        .app
        .startup
        .items
        .iter()
        .any(|item| item.detail.contains("missing-capture-plugin")));
    assert!(fixture
        .app
        .startup
        .items
        .iter()
        .any(|item| item.detail.contains("missing-control-ble-plugin")));
}

#[test]
fn host_app_displays_partially_discovered_inventory_rows() {
    let mut app = HostDesktopApp::new();
    app.apply_inventory_snapshot(partially_discovered_inventory());

    assert_eq!(app.available_device_ids, vec!["bt:AA-BB"]);
    assert_eq!(app.fleet.rows.len(), 1);
    assert_eq!(app.fleet.rows[0].device_name, "Alice iPhone");
    assert!(!app.fleet.rows[0].start_enabled);
    assert_eq!(app.selected_device_id.as_deref(), Some("bt:AA-BB"));
    assert_eq!(app.device_detail.device_name, "Alice iPhone");
    assert!(app
        .device_detail
        .inventory_notes
        .iter()
        .any(|note| note.contains("paired over bluetooth")));
    assert_eq!(app.session.ui_state, SessionUiState::Error("No capture path observed".into()));
}

#[test]
fn host_app_surfaces_observed_capture_sources_for_bluetooth_only_rows() {
    let mut app = HostDesktopApp::new();
    app.apply_inventory_snapshot(bluetooth_and_unlinked_mirror_inventory());
    app.select_device("bt:AA-BB");

    assert_eq!(
        app.device_detail.capture_sources,
        vec![CaptureSourceOption::new("window-helper-1", "Operator Mirror")]
    );
}

#[test]
fn host_app_can_link_bluetooth_row_to_observed_capture_source_and_persist_identity() {
    let mut fixture = host_app_with_runtime_and_preferences("{}");
    fixture
        .app
        .apply_inventory_snapshot(bluetooth_and_unlinked_mirror_inventory());
    fixture.app.select_device("bt:AA-BB");
    fixture.app.select_capture_source("window-helper-1");

    assert!(fixture.app.session.can_start());

    fixture.app.request_start_session();

    let prefs_path = fixture.preferences_path.as_ref().unwrap();
    let prefs = HostPreferencesStore::new(prefs_path.clone()).load().unwrap();
    let linked = prefs
        .known_devices
        .iter()
        .find(|device| device.known_device_id == "bt:AA-BB")
        .expect("expected linked bluetooth device preference");
    assert_eq!(linked.stable_id.as_deref(), Some("bt:AA-BB"));
    assert_eq!(linked.last_source_id.as_deref(), Some("window-helper-1"));
}

#[test]
fn host_app_records_inventory_diagnostic_metrics_and_logs() {
    let mut app = HostDesktopApp::new();
    app.apply_inventory_snapshot(bluetooth_and_unlinked_mirror_inventory());

    let diagnostics = format!("{:?}", app.diagnostics);
    assert!(diagnostics.contains("inventory_refreshes: 1"), "{diagnostics}");
    assert!(diagnostics.contains("inventory_rows: 2"), "{diagnostics}");
    assert!(diagnostics.contains("inventory_startable_rows: 1"), "{diagnostics}");
    assert!(
        diagnostics.contains("inventory snapshot total=2 startable=1 blocked=1"),
        "{diagnostics}"
    );
}

#[test]
fn host_app_records_session_start_diagnostic_metrics_and_logs() {
    let mut fixture = host_app_with_runtime_and_preferences("{}");
    fixture
        .app
        .apply_inventory_snapshot(bluetooth_and_unlinked_mirror_inventory());
    fixture.app.select_device("bt:AA-BB");
    fixture.app.select_capture_source("window-helper-1");
    fixture.app.request_start_session();

    let diagnostics = format!("{:?}", fixture.app.diagnostics);
    assert!(diagnostics.contains("session_start_attempts: 1"), "{diagnostics}");
    assert!(diagnostics.contains("session_start_successes: 1"), "{diagnostics}");
    assert!(
        diagnostics.contains("session start succeeded device=bt:AA-BB source=window-helper-1"),
        "{diagnostics}"
    );
}

#[test]
fn host_app_writes_launch_logs_into_user_data_logs_folder() {
    let mut fixture = host_app_with_runtime_and_preferences("{}");
    fixture
        .app
        .apply_inventory_snapshot(bluetooth_and_unlinked_mirror_inventory());
    fixture.app.select_device("bt:AA-BB");
    fixture.app.select_capture_source("window-helper-1");
    fixture.app.request_start_session();

    let prefs_path = fixture.preferences_path.as_ref().unwrap();
    let logs_dir = HostPreferencesStore::log_dir_for_preferences_path(prefs_path);
    let entries = std::fs::read_dir(&logs_dir)
        .expect("logs dir should exist")
        .collect::<Result<Vec<_>, _>>()
        .expect("logs dir entries should read");
    let log_texts = entries
        .into_iter()
        .map(|entry| std::fs::read_to_string(entry.path()).expect("launch log should be readable"))
        .collect::<Vec<_>>();
    assert!(!log_texts.is_empty(), "expected at least one launch log file");
    assert!(
        log_texts.iter().any(|text| text.contains("startup probe")),
        "{log_texts:?}"
    );
    assert!(
        log_texts.iter().any(|text| text.contains("inventory snapshot")),
        "{log_texts:?}"
    );
    assert!(
        log_texts
            .iter()
            .any(|text| text.contains("session start succeeded device=bt:AA-BB source=window-helper-1")),
        "{log_texts:?}"
    );
}

#[test]
fn host_app_merges_runtime_sessions_with_inventory_rows() {
    let mut app = HostDesktopApp::new();
    app.apply_inventory_snapshot(aggregate_inventory(vec![DeviceObservation {
        provider: InventoryEvidenceSource::Mirror,
        stable_id: None,
        known_device_id: Some("device-1".into()),
        display_name: "Operator Mirror".into(),
        mirror_source_id: Some("window-helper-1".into()),
        live: true,
        capture_state: CapabilityState::Ready,
        preferred_control_state: CapabilityState::Unavailable,
        fallback_control_state: CapabilityState::Ready,
        reasons: vec![],
    }]));
    app.apply_runtime_snapshot(runtime_snapshot_streaming_without_frame());

    assert_eq!(app.fleet.rows.len(), 1);
    assert!(app.fleet.rows[0].active_session);
    assert!(app
        .fleet
        .rows[0]
        .evidence_badges
        .iter()
        .any(|badge| badge == "Active"));
}

#[test]
fn successful_runtime_start_persists_known_device_history() {
    let mut fixture = host_app_with_runtime_and_preferences("{}");
    fixture.app.select_device("window-helper-1");

    fixture.app.request_start_session();

    let prefs_path = fixture.preferences_path.as_ref().unwrap();
    let prefs = HostPreferencesStore::new(prefs_path.clone()).load().unwrap();
    assert!(prefs
        .known_devices
        .iter()
        .any(|device| device.known_device_id == "window-helper-1"));
}

#[test]
fn host_app_without_runtime_reports_runtime_unavailable() {
    let mut app = HostDesktopApp::new();

    app.request_start_session();
    assert_eq!(
        app.session.ui_state,
        SessionUiState::Error("Host runtime unavailable".into())
    );
    assert!(app.session.selected_source.is_none());
    assert!(app.session.latest_frame.is_none());
    assert_eq!(app.device_detail.active_source_id, None);
    assert_eq!(
        app.diagnostics.host_error.as_deref(),
        Some("Host runtime unavailable")
    );
    assert_eq!(app.diagnostics.control_summary, "control blocked");
    assert_eq!(app.diagnostics.grounding_summary, "grounding blocked");

    app.stop_session();
    assert_eq!(app.session.ui_state, SessionUiState::Idle);
    assert!(app.session.selected_source.is_none());
    assert!(app.session.latest_frame.is_none());
    assert_eq!(app.device_detail.active_source_id, None);
    assert_eq!(app.diagnostics.control_summary, "control not started");
    assert_eq!(app.diagnostics.grounding_summary, "grounding idle");
}

#[test]
fn host_app_without_runtime_stays_unavailable_even_without_capture_sources() {
    let mut app = HostDesktopApp::new();
    app.device_detail.capture_sources.clear();

    app.request_start_session();

    assert_eq!(
        app.session.ui_state,
        SessionUiState::Error("Host runtime unavailable".into())
    );
    assert!(app.session.selected_source.is_none());
    assert!(app.session.latest_frame.is_none());
    assert_eq!(
        app.diagnostics.host_error.as_deref(),
        Some("Host runtime unavailable")
    );
    assert!(app.diagnostics.control_summary.contains("blocked"));
    assert!(app.diagnostics.grounding_summary.contains("blocked"));
}

#[test]
fn start_session_without_runtime_with_selected_device_reports_runtime_unavailable() {
    let mut app = HostDesktopApp::new();
    app.select_device("device-1");

    app.request_start_session();

    assert_eq!(
        app.session.ui_state,
        SessionUiState::Error("Host runtime unavailable".into())
    );
}

#[test]
fn host_app_start_session_uses_real_runtime_snapshot() {
    let mut fixture = host_app_with_runtime();
    let device_id = first_discovered_device_id(&fixture.app);
    fixture.app.select_device(&device_id);

    fixture.app.request_start_session();

    assert!(matches!(
        fixture.app.session.ui_state,
        SessionUiState::Streaming | SessionUiState::Error(_)
    ));
    assert_ne!(
        fixture.app.diagnostics.host_error.as_deref(),
        Some("Host runtime unavailable")
    );
    assert!(!fixture.app.device_detail.capture_sources.is_empty());
}

#[test]
fn host_app_start_session_forwards_selected_source_to_runtime() {
    let mut fixture = host_app_with_runtime();
    let device_id = first_discovered_device_id(&fixture.app);
    fixture.app.select_device(&device_id);
    fixture.app.device_detail.capture_sources =
        vec![CaptureSourceOption::new("missing-source", "Broken Source")];
    fixture.app.device_detail.active_source_id = Some("missing-source".into());

    fixture.app.request_start_session();

    match &fixture.app.session.ui_state {
        SessionUiState::Error(message) => {
            assert!(message.contains("missing-source"));
            assert!(message.contains("unavailable"));
        }
        other => panic!("expected runtime start failure, got {other:?}"),
    }
}

#[test]
fn host_app_restores_selected_device_from_preferences_on_launch() {
    let prefs_path = support::write_preferences_json(
        r#"{"selected_device_id":"device-2","selected_source_id":"window-helper-1"}"#,
    );
    let mut app = HostDesktopApp::with_runtime_and_preferences(
        HostRuntimeConfig {
            plugin_paths: support::host_plugin_paths(&support::workspace_root()),
        },
        host_desktop::preferences::HostPreferencesStore::new(prefs_path),
    );

    app.replace_runtime_statuses(vec![
        status(
            "device-1",
            "Alpha",
            SessionPhase::Streaming,
            SessionSubstate::ControlReady,
            "capture.window.helper",
            "control.window-bridge",
            None,
        ),
        status(
            "device-2",
            "Beta",
            SessionPhase::Streaming,
            SessionSubstate::ControlReady,
            "capture.window.helper",
            "control.window-bridge",
            None,
        ),
    ]);

    assert_eq!(app.selected_device_id.as_deref(), Some("device-2"));
}

#[test]
fn host_app_uses_persisted_source_preference_when_starting_session() {
    let mut fixture = host_app_with_runtime_and_preferences(
        r#"{"selected_device_id":"device-1","selected_source_id":"missing-source"}"#,
    );
    fixture.app.replace_runtime_statuses(vec![status(
        "device-1",
        "Alpha",
        SessionPhase::Streaming,
        SessionSubstate::ControlReady,
        "capture.direct.fake",
        "control.window-bridge",
        None,
    )]);

    fixture.app.request_start_session();

    assert!(matches!(
        fixture.app.session.ui_state,
        SessionUiState::Streaming | SessionUiState::Error(_)
    ));
    let prefs_path = fixture
        .preferences_path
        .as_ref()
        .expect("preferences path should be captured");
    let saved = HostPreferencesStore::new(prefs_path.clone())
        .load()
        .expect("preferences should load");
    assert_ne!(saved.selected_source_id.as_deref(), Some("missing-source"));
}

#[test]
fn host_app_manual_capture_source_selection_overrides_restored_source_on_start() {
    let mut fixture = host_app_with_runtime_and_preferences(
        r#"{"selected_device_id":"device-1","selected_source_id":"missing-source"}"#,
    );
    fixture.app.replace_runtime_statuses(vec![status(
        "device-1",
        "Alpha",
        SessionPhase::Streaming,
        SessionSubstate::ControlReady,
        "capture.window.helper",
        "control.window-bridge",
        None,
    )]);
    fixture.app.select_capture_source("window-helper-1");

    fixture.app.request_start_session();

    assert!(matches!(
        fixture.app.session.ui_state,
        SessionUiState::Streaming | SessionUiState::Error(_)
    ));
    let prefs_path = fixture
        .preferences_path
        .as_ref()
        .expect("preferences path should be captured");
    let saved = HostPreferencesStore::new(prefs_path.clone())
        .load()
        .expect("preferences should load");
    assert_eq!(saved.selected_source_id.as_deref(), Some("window-helper-1"));
}

#[test]
fn host_app_preflight_clears_unavailable_restored_source_before_start_call() {
    let mut fixture = host_app_with_missing_runtime_plugins_and_preferences(
        r#"{"selected_device_id":"device-1","selected_source_id":"missing-source"}"#,
    );
    fixture.app.replace_runtime_statuses(vec![status(
        "device-1",
        "Alpha",
        SessionPhase::Streaming,
        SessionSubstate::ControlReady,
        "capture.direct.fake",
        "control.window-bridge",
        None,
    )]);
    assert_eq!(fixture.app.device_detail.active_source_id.as_deref(), Some("direct-1"));

    fixture.app.request_start_session();

    let prefs_path = fixture
        .preferences_path
        .as_ref()
        .expect("preferences path should be captured");
    let saved = HostPreferencesStore::new(prefs_path.clone())
        .load()
        .expect("preferences should load");
    assert_eq!(saved.selected_source_id, None);
}

#[test]
fn host_app_stop_session_removes_runtime_status() {
    let mut fixture = host_app_with_runtime();
    let device_id = first_discovered_device_id(&fixture.app);
    fixture.app.select_device(&device_id);
    fixture.app.request_start_session();

    fixture.app.stop_session();

    assert_eq!(fixture.app.available_device_ids, vec!["window-helper-1"]);
    assert_eq!(fixture.app.session.ui_state, SessionUiState::Idle);
}

#[test]
fn selecting_a_capture_source_updates_device_detail_selection() {
    let mut fixture = host_app_with_runtime();
    let device_id = first_discovered_device_id(&fixture.app);
    fixture.app.select_device(&device_id);
    fixture.app.request_start_session();

    fixture.app.select_capture_source("window-helper-1");

    assert_eq!(
        fixture.app.device_detail.active_source_id.as_deref(),
        Some("window-helper-1")
    );
}

#[test]
fn runtime_snapshot_populates_control_checklist_and_operator_message() {
    let mut fixture = host_app_with_runtime();
    let device_id = first_discovered_device_id(&fixture.app);
    fixture.app.select_device(&device_id);
    fixture.app.request_start_session();

    assert_ne!(
        fixture.app.device_detail.control_checklist,
        ControlSetupChecklist::for_pointer_mode()
    );
    assert_ne!(
        fixture.app.diagnostics.control_summary,
        "control backend control.ble"
    );
    assert!(fixture.app.diagnostics.control_summary.contains("control "));
}

#[test]
fn runtime_snapshot_preserves_control_phase_and_observed_change() {
    let snapshot = runtime_snapshot_with_control(
        ControlSessionPhase::Advertising,
        "Waiting for iPhone",
        Some(true),
    );

    assert_eq!(
        snapshot.workspace.control_phase,
        ControlSessionPhase::Advertising
    );
    assert_eq!(snapshot.workspace.execution_observed_change, Some(true));
}

#[test]
fn host_app_uses_runtime_frame_metadata_for_streaming_state() {
    let snapshot =
        runtime_snapshot_with_frame(ios_control_contracts::capture::VideoFrameDescriptor {
            source_id: "window-helper-1".into(),
            source_kind: ios_control_contracts::capture::SourceKind::Window,
            width: 640,
            height: 360,
            rotation_degrees: 90,
            frame_index: 8,
            health: ios_control_contracts::capture::FrameHealth::Occluded,
        });

    let app = host_app_from_runtime_snapshot(snapshot);

    assert_eq!(app.session.latest_frame.as_ref().unwrap().width, 640);
    assert_eq!(app.session.latest_frame.as_ref().unwrap().height, 360);
    assert_eq!(
        app.session.latest_frame.as_ref().unwrap().rotation_degrees,
        90
    );
    assert_eq!(
        app.session.latest_frame.as_ref().unwrap().health,
        ios_control_contracts::capture::FrameHealth::Occluded
    );
}

#[test]
fn host_app_keeps_streaming_ui_state_when_runtime_frame_is_not_ready_yet() {
    let app = host_app_from_runtime_snapshot(runtime_snapshot_streaming_without_frame());

    assert_eq!(app.session.ui_state, SessionUiState::Streaming);
    assert!(app.session.latest_frame.is_none());
}

#[test]
fn host_app_surfaces_reconnect_guidance_for_degraded_control() {
    let app = host_app_from_runtime_snapshot(runtime_snapshot_with_control(
        ControlSessionPhase::Error,
        "Reconnect BLE helper",
        Some(false),
    ));

    assert!(app
        .diagnostics
        .control_summary
        .contains("Reconnect BLE helper"));
}

#[test]
fn startup_with_selected_device_attempts_start_and_reports_runtime_unavailable() {
    let mut app = HostDesktopApp::new();
    app.select_device("device-1");

    app.start_runtime_session_on_launch();

    assert_eq!(app.selected_device_id.as_deref(), Some("device-1"));
    assert_eq!(
        app.session.ui_state,
        SessionUiState::Error("Host runtime unavailable".into())
    );
}

#[test]
fn stop_session_clears_runtime_state_even_without_runtime_instance() {
    let mut app = HostDesktopApp::new();
    app.replace_runtime_statuses(vec![status(
        "device-1",
        "Alpha",
        SessionPhase::Streaming,
        SessionSubstate::Streaming,
        "capture.window.helper",
        "control.ble",
        None,
    )]);

    app.stop_session();

    assert!(app.available_device_ids.is_empty());
    assert!(app.fleet.rows.is_empty());
    assert_eq!(app.dashboard.total_devices, 0);
    assert_eq!(app.dashboard.degraded_devices, 0);
    assert!(app.settings.plugin_rows.is_empty());
    assert!(app.selected_device_id.is_none());
    assert_eq!(app.session.ui_state, SessionUiState::Idle);
}

#[test]
fn app_tracks_selected_workspace_separately_from_fleet_rows() {
    let mut app = HostDesktopApp::new();
    app.selected_device_id = Some("device-2".into());
    app.available_device_ids = vec!["device-1".into(), "device-2".into()];

    assert_eq!(app.selected_device_id.as_deref(), Some("device-2"));
    assert_eq!(app.available_device_ids.len(), 2);
}

fn status(
    device_id: &str,
    device_name: &str,
    phase: SessionPhase,
    substate: SessionSubstate,
    capture_backend: &str,
    control_backend: &str,
    operator_action: Option<&str>,
) -> DeviceSessionStatus {
    DeviceSessionStatus::new(
        DeviceSessionSummary {
            device_id: device_id.into(),
            device_name: device_name.into(),
            phase,
            plugin_health: if phase == SessionPhase::Degraded {
                PluginHealth::Degraded
            } else {
                PluginHealth::Healthy
            },
            capture_plugin: Some(capture_backend.into()),
            control_plugin: Some(control_backend.into()),
            grounding_plugin: Some("grounding.core".into()),
        },
        substate,
        BackendSelection {
            capture_backend: capture_backend.into(),
            control_backend: control_backend.into(),
        },
        operator_action.map(str::to_string),
    )
    .expect("valid session status")
}

#[test]
fn app_syncs_runtime_statuses_into_fleet_and_workspace() {
    let mut app = HostDesktopApp::new();
    app.replace_runtime_statuses(vec![
        status(
            "device-1",
            "Alpha",
            SessionPhase::Streaming,
            SessionSubstate::ControlReady,
            "capture.window.helper",
            "control.ble",
            None,
        ),
        status(
            "device-2",
            "Beta",
            SessionPhase::Degraded,
            SessionSubstate::OperatorActionRequired,
            "capture.window.helper",
            "control.window-bridge",
            Some("reconnect mirror helper"),
        ),
    ]);

    assert_eq!(app.dashboard.total_devices, 2);
    assert_eq!(app.dashboard.degraded_devices, 1);
    assert_eq!(app.available_device_ids, vec!["device-1", "device-2"]);
    assert_eq!(app.selected_device_id.as_deref(), Some("device-1"));
    assert_eq!(app.fleet.rows.len(), 2);
    assert_eq!(app.device_detail.device_name, "Alpha");
    assert_eq!(app.session.ui_state, SessionUiState::Streaming);
    assert!(app.diagnostics.host_error.is_none());
    assert!(app
        .settings
        .plugin_rows
        .iter()
        .any(|row| row.contains("control.window-bridge")));
}

#[test]
fn selecting_a_device_updates_workspace_and_operator_error() {
    let mut app = HostDesktopApp::new();
    app.replace_runtime_statuses(vec![
        status(
            "device-1",
            "Alpha",
            SessionPhase::Streaming,
            SessionSubstate::ControlReady,
            "capture.window.helper",
            "control.ble",
            None,
        ),
        status(
            "device-2",
            "Beta",
            SessionPhase::Degraded,
            SessionSubstate::OperatorActionRequired,
            "capture.window.helper",
            "control.window-bridge",
            Some("reconnect mirror helper"),
        ),
    ]);

    app.select_device("device-2");

    assert_eq!(app.selected_device_id.as_deref(), Some("device-2"));
    assert_eq!(app.device_detail.device_name, "Beta");
    assert_eq!(
        app.session.ui_state,
        SessionUiState::Error("reconnect mirror helper".into())
    );
    assert_eq!(
        app.diagnostics.host_error.as_deref(),
        Some("reconnect mirror helper")
    );
    assert!(app
        .diagnostics
        .control_summary
        .contains("control.window-bridge"));
}
