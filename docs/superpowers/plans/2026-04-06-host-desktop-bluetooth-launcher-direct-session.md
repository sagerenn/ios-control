# Host Desktop Bluetooth Launcher Direct Session Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert `host-desktop` into a Bluetooth-device launcher with read-only settings and a direct-receiver-backed session window that can wait for mirroring before showing streaming content.

**Architecture:** Reuse the existing bootstrap, inventory, preferences, and logging layers, but add launcher-specific filtering and settings rows in the app/view-model layer. The root egui viewport becomes a launcher plus settings surface, while a second viewport renders the selected device session. The direct runtime path is updated so a session can exist without a first frame yet, allowing the app to delay the session window for startable devices until mirroring is actually live.

**Tech Stack:** Rust, eframe/egui 0.31, existing `host-desktop` tests, existing `ios-control-session-orchestrator` tests

---

### File Structure

- Create: `apps/host-desktop/src/panels/launcher.rs`
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/src/lib.rs`
- Modify: `apps/host-desktop/src/panels/session_view.rs`
- Modify: `apps/host-desktop/src/panels/settings.rs`
- Modify: `apps/host-desktop/src/view_models/fleet.rs`
- Modify: `apps/host-desktop/src/view_models/session.rs`
- Modify: `apps/host-desktop/src/view_models/settings.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`
- Modify: `apps/host-desktop/tests/fleet_view_model.rs`
- Modify: `apps/host-desktop/tests/runtime_integration.rs`
- Modify: `crates/session-orchestrator/src/lib.rs`
- Modify: `crates/session-orchestrator/src/session_actor.rs`
- Modify: `crates/session-orchestrator/tests/mock_flow.rs`
- Modify: `crates/session-orchestrator/tests/support/mod.rs`

### Task 1: Add Failing Launcher And Settings Tests

**Files:**
- Modify: `apps/host-desktop/tests/fleet_view_model.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`
- Modify: `apps/host-desktop/src/view_models/settings.rs`

- [ ] **Step 1: Write the failing launcher filtering test**

```rust
#[test]
fn fleet_view_model_launcher_filters_to_bluetooth_rows() {
    let inventory = aggregate_inventory(vec![
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
            reasons: vec!["paired over bluetooth".into()],
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
    ]);

    let fleet = FleetViewModel::for_launcher(&inventory.devices, true, &[]);
    assert_eq!(fleet.rows.len(), 1);
    assert_eq!(fleet.rows[0].device_name, "Alice iPhone");
    assert_eq!(fleet.rows[0].readiness_summary, "Startable");
    assert!(fleet.rows[0].start_enabled);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p host-desktop fleet_view_model_launcher_filters_to_bluetooth_rows -- --exact`
Expected: FAIL with missing `FleetViewModel::for_launcher`

- [ ] **Step 3: Write the failing settings-path test**

```rust
#[test]
fn host_app_settings_surface_preferences_and_log_paths() {
    let fixture = host_app_with_runtime_and_preferences("{}");
    let rows = &fixture.app.settings.rows;

    let prefs_path = fixture.preferences_path.as_ref().unwrap();
    let logs_dir = HostPreferencesStore::log_dir_for_preferences_path(prefs_path);

    assert!(rows.iter().any(|row| row.contains(&prefs_path.display().to_string())));
    assert!(rows.iter().any(|row| row.contains(&logs_dir.display().to_string())));
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cargo test -p host-desktop host_app_settings_surface_preferences_and_log_paths -- --exact`
Expected: FAIL because `SettingsViewModel` does not expose path rows yet

### Task 2: Add Failing Session Window And Waiting-State Tests

**Files:**
- Modify: `apps/host-desktop/tests/app_state.rs`
- Modify: `crates/session-orchestrator/tests/mock_flow.rs`
- Modify: `crates/session-orchestrator/tests/support/mod.rs`

- [ ] **Step 1: Write the failing app-state test for deferred session window visibility**

```rust
#[test]
fn host_app_defers_startable_session_window_until_direct_frame_is_live() {
    let mut app = HostDesktopApp::new();
    app.apply_inventory_snapshot(aggregate_inventory(vec![DeviceObservation {
        provider: InventoryEvidenceSource::Bluetooth,
        stable_id: Some("bt:AA-BB".into()),
        known_device_id: None,
        display_name: "Alice iPhone".into(),
        mirror_source_id: None,
        live: true,
        capture_state: CapabilityState::Unavailable,
        preferred_control_state: CapabilityState::Ready,
        fallback_control_state: CapabilityState::Unavailable,
        reasons: vec!["paired over bluetooth".into()],
    }]));
    app.apply_direct_receiver_availability_for_tests(true, "receiver ready");
    app.select_device("bt:AA-BB");

    app.request_open_selected_device_session();
    assert!(!app.session_window_is_visible());

    app.apply_runtime_snapshot(direct_waiting_snapshot("bt:AA-BB", "Alice iPhone"));
    assert!(!app.session_window_is_visible());

    app.apply_runtime_snapshot(direct_streaming_snapshot("bt:AA-BB", "Alice iPhone"));
    assert!(app.session_window_is_visible());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p host-desktop host_app_defers_startable_session_window_until_direct_frame_is_live -- --exact`
Expected: FAIL with missing launcher/session-window methods

- [ ] **Step 3: Write the failing direct-wait test**

```rust
#[tokio::test]
async fn start_session_with_direct_backend_can_wait_for_first_frame() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let helper = write_direct_helper(
        r#"#!/bin/sh
if [ "$1" = "probe" ]; then
  echo '{"available":true,"supports_input_bridge":false}'
  exit 0
fi
if [ "$1" = "stream" ]; then
  sleep 3
  exit 0
fi
exit 2
"#,
    );
    let _env = EnvVarGuards::new(vec![EnvVarGuard::set(
        "IOS_CONTROL_DIRECT_RECEIVER_HELPER",
        &helper,
    )]);

    let mut orchestrator = SessionOrchestrator::default();
    let state = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "direct-wait".into(),
            device_name: "Alice iPhone".into(),
            selected_source_id: Some("direct-1".into()),
            capture_backend: CaptureBackend::Direct,
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                capture_direct: plugin_path(&root, "plugin-capture-direct"),
                control_ble: plugin_path(&root, "plugin-control-ble"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    assert_eq!(state.summary.phase, SessionPhase::Connecting);
    assert!(state.capture_stream.is_some());
    assert!(state.latest_frame.is_none());
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cargo test -p ios-control-session-orchestrator start_session_with_direct_backend_can_wait_for_first_frame -- --exact`
Expected: FAIL because direct startup still errors on first-frame timeout

### Task 3: Implement Launcher Rows, Settings Rows, And Session Window State

**Files:**
- Create: `apps/host-desktop/src/panels/launcher.rs`
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/src/lib.rs`
- Modify: `apps/host-desktop/src/panels/session_view.rs`
- Modify: `apps/host-desktop/src/panels/settings.rs`
- Modify: `apps/host-desktop/src/view_models/fleet.rs`
- Modify: `apps/host-desktop/src/view_models/session.rs`
- Modify: `apps/host-desktop/src/view_models/settings.rs`

- [ ] **Step 1: Add launcher-specific fleet construction**

```rust
pub fn for_launcher(
    devices: &[InventoryDevice],
    direct_receiver_available: bool,
    statuses: &[DeviceSessionStatus],
) -> Self {
    let rows = devices
        .iter()
        .filter(|device| device.evidence_sources.contains(&InventoryEvidenceSource::Bluetooth))
        .map(|device| launcher_row(device, direct_receiver_available, statuses))
        .collect();
    Self { rows }
}
```

- [ ] **Step 2: Replace settings plugin rows with read-only path rows**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsViewModel {
    pub rows: Vec<String>,
}
```

- [ ] **Step 3: Add a dedicated launcher panel with double-click handling**

```rust
pub enum LauncherAction {
    None,
    OpenDevice(String),
}

pub fn render(ui: &mut Ui, fleet: &FleetViewModel, selected_device_id: Option<&str>) -> LauncherAction {
    let mut action = LauncherAction::None;
    ui.heading("Devices");
    for row in &fleet.rows {
        let response = ui.selectable_label(
            Some(row.device_id.as_str()) == selected_device_id,
            format!("{} | {}", row.device_name, row.readiness_summary),
        );
        if response.double_clicked() {
            action = LauncherAction::OpenDevice(row.device_id.clone());
        }
    }
    action
}
```

- [ ] **Step 4: Extend the session view-model with waiting and blocked states**

```rust
pub enum SessionUiState {
    Idle,
    WaitingForMirror,
    Streaming,
    Blocked(String),
    Error(String),
}
```

- [ ] **Step 5: Add explicit session-window state to the host app**

```rust
struct SessionWindowState {
    open: bool,
    deferred_until_streaming: bool,
    device_id: Option<String>,
}
```

- [ ] **Step 6: Render only launcher plus settings in the root viewport and render the session viewport separately**

```rust
egui::CentralPanel::default().show(ctx, |ui| {
    launcher_action = launcher::render(ui, &self.fleet, self.selected_device_id.as_deref());
    ui.separator();
    settings::render_rows(ui, &self.settings.rows);
});
```

- [ ] **Step 7: Re-run targeted launcher and app-state tests**

Run: `cargo test -p host-desktop fleet_view_model_launcher_filters_to_bluetooth_rows -- --exact`
Expected: PASS

Run: `cargo test -p host-desktop host_app_settings_surface_preferences_and_log_paths -- --exact`
Expected: PASS

Run: `cargo test -p host-desktop host_app_defers_startable_session_window_until_direct_frame_is_live -- --exact`
Expected: still FAIL until the direct wait-state runtime work lands

### Task 4: Implement Direct Wait-For-Frame Runtime Behavior

**Files:**
- Modify: `crates/session-orchestrator/src/lib.rs`
- Modify: `crates/session-orchestrator/src/session_actor.rs`
- Modify: `apps/host-desktop/src/runtime.rs`
- Modify: `apps/host-desktop/tests/runtime_integration.rs`
- Modify: `crates/session-orchestrator/tests/mock_flow.rs`
- Modify: `crates/session-orchestrator/tests/support/mod.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`

- [ ] **Step 1: Split direct stream opening from initial frame acquisition**

```rust
async fn open_capture_stream(
    capture: &mut RunningPlugin,
    selected_source_id: &str,
) -> Result<CaptureStreamDescriptor> {
    match request_plugin(
        capture,
        &HostToPlugin::OpenCaptureStream {
            source_id: selected_source_id.into(),
        },
    )
    .await? {
        PluginToHost::CaptureStreamOpened { stream } => Ok(stream),
        other => Err(anyhow!("unexpected capture stream response: {other:?}")),
    }
}
```

- [ ] **Step 2: Treat direct first-frame timeouts as a waiting session instead of a fatal startup error**

```rust
let capture_stream = Some(open_capture_stream(&mut capture, &selected_source_id).await?);
let latest_frame = match request.capture_backend {
    CaptureBackend::Direct => match read_capture_frame(&mut capture).await {
        Ok(frame) => Some(frame),
        Err(error) if error.to_string().contains("timed out") => None,
        Err(error) => return Err(error),
    },
    CaptureBackend::Window => Some(read_capture_frame(&mut capture).await?),
};
```

- [ ] **Step 3: Start direct sessions in `Connecting` when no frame exists and promote them to `Streaming` on refresh**

```rust
if self.latest_frame.is_none() {
    match read_capture_frame(capture).await {
        Ok(frame) => {
            self.latest_frame = Some(frame.clone());
            self.summary.phase = SessionPhase::Streaming;
            return Ok(Some(frame));
        }
        Err(error) if error.to_string().contains("timed out") => return Ok(None),
        Err(error) => return Err(error),
    }
}
```

- [ ] **Step 4: Keep the host app polling waiting direct sessions and reveal the deferred session window once streaming arrives**

```rust
if self.session_window.deferred_until_streaming && self.session.latest_frame.is_some() {
    self.session_window.open = true;
    self.session_window.deferred_until_streaming = false;
}
```

- [ ] **Step 5: Re-run targeted runtime and app-state tests**

Run: `cargo test -p ios-control-session-orchestrator start_session_with_direct_backend_can_wait_for_first_frame -- --exact`
Expected: PASS

Run: `cargo test -p host-desktop host_app_defers_startable_session_window_until_direct_frame_is_live -- --exact`
Expected: PASS

- [ ] **Step 6: Run full package verification**

Run: `cargo test -p ios-control-session-orchestrator -p host-desktop`
Expected: PASS
