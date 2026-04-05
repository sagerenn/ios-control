# Audited Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining audited gaps in host runtime truth, live preview transport, control backend selection, and validation/doc honesty without regressing the existing mock-backed developer flow.

**Architecture:** Keep the current Rust workspace and plugin boundaries, but treat the remaining work as four isolated gap-closure tracks. Make the host runtime authoritative first, then move real frame metadata and refresh through that runtime, then pick the actual control backend at orchestration time, and finally tighten the status docs and validation workflow so the branch cannot overstate its real-device state.

**Tech Stack:** Rust, tokio, eframe/egui, JSON stdio plugins, Cargo integration tests, Python unittest, Markdown docs

---

## File Structure

- `apps/host-desktop/src/runtime.rs`: remove the legacy in-memory runtime bridge and expose runtime refresh APIs that return authoritative session snapshots.
- `apps/host-desktop/src/app.rs`: stop simulating session startup, preserve selected capture source, poll refreshed runtime snapshots, and stop inventing frame metadata.
- `apps/host-desktop/src/main.rs`: keep app bootstrap minimal and runtime-backed.
- `apps/host-desktop/src/view_models/session.rs`: carry real frame metadata and degraded-state text into the session view.
- `apps/host-desktop/src/panels/session_view.rs`: render preview metadata from runtime frames rather than only fallback summaries.
- `apps/host-desktop/tests/app_state.rs`: host-level regressions for runtime-backed session start, selected-source forwarding, and runtime-frame-driven UI state.
- `apps/host-desktop/tests/runtime_integration.rs`: runtime-level regressions for refreshed frames and authoritative snapshots.
- `plugins/capture-window/src/helper_bridge.rs`: decode helper RGBA payloads plus rotation/health metadata.
- `plugins/capture-window/src/main.rs`: require real RGBA payloads for helper-backed stream reads and propagate frame metadata.
- `plugins/capture-window/tests/window_contract.rs`: capture-helper regressions for RGBA payload decoding and metadata propagation.
- `plugins/capture-direct/src/helper_bridge.rs`: decode direct-receiver RGBA payloads plus rotation/health metadata.
- `plugins/capture-direct/src/main.rs`: require real RGBA payloads for helper-backed direct capture reads and propagate frame metadata.
- `plugins/capture-direct/tests/direct_receiver_contract.rs`: direct-capture regressions for RGBA payload decoding and metadata propagation.
- `crates/session-orchestrator/src/lib.rs`: expose mutable active-session refresh, choose the real control backend, and mark observed execution as applied when appropriate.
- `crates/session-orchestrator/src/session_actor.rs`: derive status snapshots from the actual selected control backend.
- `crates/session-orchestrator/tests/mock_flow.rs`: regressions for runtime frame refresh and observed execution semantics.
- `crates/session-orchestrator/tests/fallback_flow.rs`: regression for real fallback backend selection.
- `crates/session-orchestrator/tests/support/mod.rs`: helper fixtures for BLE helper scripts used by orchestrator tests.
- `tests/ci/test_docs_status.py`: stronger guards for current-reality wording and validation evidence rules.
- `docs/TODO.md`: audited status summary and plan links.
- `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`: non-mock validation evidence rules and row updates only after dated records exist.
- `docs/validation/real-device-session-template.md`: manual validation template.
- `docs/validation/2026-04-04-linux-window-ble.md`: first Linux BLE validation record after a completed run.
- `docs/validation/2026-04-04-linux-window-window-input.md`: first Linux fallback validation record after a completed run.
- `docs/validation/2026-04-04-windows-window-ble.md`: first Windows BLE validation record after a completed run.
- `docs/validation/2026-04-04-windows-window-window-input.md`: first Windows fallback validation record after a completed run.

### Task 1: Remove Legacy Host Bootstrap State And Make Runtime State Authoritative

**Files:**
- Modify: `apps/host-desktop/src/runtime.rs`
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/src/main.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`
- Test: `apps/host-desktop/tests/app_state.rs`

- [ ] **Step 1: Write the failing host-app regression tests**

```rust
#[test]
fn host_app_without_runtime_reports_runtime_unavailable() {
    let mut app = HostDesktopApp::new();

    app.request_start_session();

    assert_eq!(
        app.session.ui_state,
        SessionUiState::Error("Host runtime unavailable".into())
    );
    assert_eq!(app.diagnostics.host_error.as_deref(), Some("Host runtime unavailable"));
    assert_eq!(app.diagnostics.control_summary, "control blocked");
    assert_eq!(app.diagnostics.grounding_summary, "grounding blocked");
}

#[test]
fn host_app_start_session_forwards_selected_source_to_runtime() {
    let mut fixture = host_app_with_runtime();
    fixture.app.select_device("device-1");
    fixture.app.device_detail.capture_sources = vec![CaptureSourceOption::new(
        "missing-source",
        "Broken Source",
    )];
    fixture.app.device_detail.active_source_id = Some("missing-source".into());

    fixture.app.request_start_session();

    assert_eq!(
        fixture.app.session.ui_state,
        SessionUiState::Error(
            "requested capture source `missing-source` is unavailable for capture.window".into()
        )
    );
}
```

- [ ] **Step 2: Run the host-app tests to verify they fail**

Run: `cargo test -p host-desktop host_app_without_runtime_reports_runtime_unavailable -- --exact`
Expected: FAIL because `request_start_session()` still enters the synthetic starting path when no runtime exists.

Run: `cargo test -p host-desktop host_app_start_session_forwards_selected_source_to_runtime -- --exact`
Expected: FAIL because `request_start_session()` clears `active_source_id` before the runtime call, so the runtime never sees `missing-source`.

- [ ] **Step 3: Remove `HostRuntimeBridge`, delete the pending-start path, and make `request_start_session()` pass the selected source directly**

```rust
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

pub fn apply_runtime_snapshot(&mut self, snapshot: HostRuntimeSnapshot) {
    self.runtime_workspace = Some(snapshot.workspace.clone());
    self.runtime_statuses = snapshot.statuses.clone();
    self.preview_texture = None;
    self.sync_from_runtime();
}
```

- [ ] **Step 4: Run the host-app test slice to verify it passes**

Run: `cargo test -p host-desktop host_app_without_runtime_reports_runtime_unavailable -- --exact`
Expected: PASS

Run: `cargo test -p host-desktop host_app_start_session_forwards_selected_source_to_runtime -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/host-desktop/src/runtime.rs \
  apps/host-desktop/src/app.rs \
  apps/host-desktop/src/main.rs \
  apps/host-desktop/tests/app_state.rs
git commit -m "feat: remove synthetic host bootstrap path"
```

### Task 2: Require Real RGBA Frame Payloads And Propagate Frame Metadata

**Files:**
- Modify: `plugins/capture-window/src/helper_bridge.rs`
- Modify: `plugins/capture-window/src/main.rs`
- Modify: `plugins/capture-window/tests/window_contract.rs`
- Modify: `plugins/capture-direct/src/helper_bridge.rs`
- Modify: `plugins/capture-direct/src/main.rs`
- Modify: `plugins/capture-direct/tests/direct_receiver_contract.rs`
- Test: `plugins/capture-window/tests/window_contract.rs`
- Test: `plugins/capture-direct/tests/direct_receiver_contract.rs`

- [ ] **Step 1: Write the failing helper-metadata tests**

```rust
#[test]
fn helper_frame_event_decodes_rgba_rotation_and_health() {
    let event: HelperFrameEvent = serde_json::from_str(
        r#"{
            "frame_index": 4,
            "width": 2,
            "height": 1,
            "rotation_degrees": 90,
            "health": "Occluded",
            "rgba_base64": "AQIDBAUGBwg="
        }"#,
    )
    .unwrap();

    assert_eq!(event.decode_rgba().unwrap(), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(event.rotation_degrees, 90);
    assert_eq!(event.health, FrameHealth::Occluded);
}
```

- [ ] **Step 2: Run the capture-helper tests to verify they fail**

Run: `cargo test -p plugin-capture-window helper_frame_event_decodes_rgba_rotation_and_health -- --exact`
Expected: FAIL because `HelperFrameEvent` still only models `fill_byte` data and does not expose rotation or health.

Run: `cargo test -p plugin-capture-direct helper_frame_event_decodes_rgba_rotation_and_health -- --exact`
Expected: FAIL for the same reason in the direct-capture path.

- [ ] **Step 3: Change both helper contracts from fill-byte fallback to explicit RGBA payload plus metadata**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HelperFrameEvent {
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub rotation_degrees: u16,
    #[serde(default = "default_frame_health")]
    pub health: FrameHealth,
    pub rgba_base64: String,
}

fn default_frame_health() -> FrameHealth {
    FrameHealth::Healthy
}

impl HelperFrameEvent {
    pub fn decode_rgba(&self) -> Result<Vec<u8>> {
        decode_base64_bytes(&self.rgba_base64)
    }
}

let bytes = event.decode_rgba().map_err(Box::<dyn Error>::from)?;
if bytes.len() != state.slot.byte_len() {
    let reply = PluginToHost::Error {
        message: format!(
            "helper frame payload size mismatch: expected {}, got {}",
            state.slot.byte_len(),
            bytes.len()
        ),
    };
    write_reply(&mut stdout, &reply)?;
    continue;
}
state.slot.write(&bytes)?;

frame.width = event.width;
frame.height = event.height;
frame.rotation_degrees = event.rotation_degrees;
frame.health = event.health;
```

- [ ] **Step 4: Update embedded helper mode so local test helpers also emit real RGBA bytes**

```rust
let rgba = base64::engine::general_purpose::STANDARD.encode([
    255_u8, 0, 0, 255, 0, 255, 0, 255,
]);
let payload = serde_json::json!({
    "frame_index": 1_u64,
    "width": 2_u32,
    "height": 1_u32,
    "rotation_degrees": 0_u16,
    "health": "Healthy",
    "rgba_base64": rgba,
});
println!("{}", serde_json::to_string(&payload)?);
```

- [ ] **Step 5: Run the capture-helper suites to verify they pass**

Run: `cargo test -p plugin-capture-window --test window_contract`
Expected: PASS

Run: `cargo test -p plugin-capture-direct --test direct_receiver_contract`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add plugins/capture-window/src/helper_bridge.rs \
  plugins/capture-window/src/main.rs \
  plugins/capture-window/tests/window_contract.rs \
  plugins/capture-direct/src/helper_bridge.rs \
  plugins/capture-direct/src/main.rs \
  plugins/capture-direct/tests/direct_receiver_contract.rs
git commit -m "feat: require rgba capture payloads and frame metadata"
```

### Task 3: Refresh Preview Frames Through The Runtime And Stop Inventing Frame Metadata

**Files:**
- Modify: `apps/host-desktop/src/runtime.rs`
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/src/view_models/session.rs`
- Modify: `apps/host-desktop/src/panels/session_view.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`
- Modify: `apps/host-desktop/tests/runtime_integration.rs`
- Modify: `crates/session-orchestrator/src/lib.rs`
- Modify: `crates/session-orchestrator/tests/mock_flow.rs`
- Test: `apps/host-desktop/tests/app_state.rs`
- Test: `apps/host-desktop/tests/runtime_integration.rs`
- Test: `crates/session-orchestrator/tests/mock_flow.rs`

- [ ] **Step 1: Write the failing runtime-refresh and host-frame tests**

```rust
#[test]
fn runtime_refresh_session_updates_workspace_latest_frame() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _guards = prepare_window_runtime_env(&root);

    let mut runtime = HostRuntime::new(HostRuntimeConfig {
        plugin_paths: host_plugin_paths(&root),
    })
    .unwrap();

    let first = runtime
        .start_session("device-1", "Mock iPhone", Some("window-helper-1".into()))
        .unwrap()
        .workspace
        .latest_frame
        .unwrap()
        .frame_index;

    let refreshed = runtime.refresh_session("device-1").unwrap();
    assert!(refreshed.workspace.latest_frame.unwrap().frame_index > first);
}

#[test]
fn host_app_uses_runtime_frame_metadata_for_streaming_state() {
    let snapshot = runtime_snapshot_with_frame(ios_control_contracts::capture::VideoFrameDescriptor {
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
    assert_eq!(app.session.latest_frame.as_ref().unwrap().rotation_degrees, 90);
    assert_eq!(
        app.session.latest_frame.as_ref().unwrap().health,
        ios_control_contracts::capture::FrameHealth::Occluded
    );
}
```

- [ ] **Step 2: Run the focused runtime/host tests to verify they fail**

Run: `cargo test -p host-desktop runtime_refresh_session_updates_workspace_latest_frame -- --exact`
Expected: FAIL because `HostRuntime` does not expose a refresh API and `RuntimeWorkspaceState` does not carry `latest_frame`.

Run: `cargo test -p host-desktop host_app_uses_runtime_frame_metadata_for_streaming_state -- --exact`
Expected: FAIL because `sync_selected_workspace()` still synthesizes a `1280x720` healthy frame instead of using runtime frame metadata.

- [ ] **Step 3: Extend runtime snapshots with `latest_frame` and add a refresh path**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWorkspaceState {
    pub device_id: String,
    pub summary: DeviceSessionSummary,
    pub capture_sources: Vec<VideoSource>,
    pub capture_stream: Option<CaptureStreamDescriptor>,
    pub latest_frame: Option<VideoFrameDescriptor>,
    pub selected_source_id: Option<String>,
    pub control_checklist: ControlSetupChecklist,
    pub control_phase: ControlSessionPhase,
    pub execution_observed_change: Option<bool>,
    pub diagnostics: SessionDiagnostics,
}

impl HostRuntime {
    pub fn refresh_session(&mut self, device_id: &str) -> Result<HostRuntimeSnapshot> {
        self.tokio
            .block_on(self.supervisor.refresh_session(device_id))?;
        self.snapshot(device_id)
    }
}

impl SessionSupervisor {
    pub async fn refresh_session(&mut self, device_id: &str) -> Result<()> {
        let active = self
            .active
            .get_mut(device_id)
            .ok_or_else(|| anyhow!("missing active session for {device_id}"))?;
        active.refresh_capture_frame().await?;
        let status = session_actor::status_snapshot(active)?;
        self.sessions.insert(device_id.into(), status);
        Ok(())
    }
}
```

- [ ] **Step 4: Update the host app to poll the runtime and build `SessionViewModel` from real frames**

```rust
fn sync_selected_workspace(&mut self) {
    let Some(workspace) = self
        .runtime_workspace
        .as_ref()
        .filter(|workspace| Some(workspace.device_id.as_str()) == self.selected_device_id.as_deref())
    else {
        return;
    };

    let Some(source) = workspace
        .selected_source_id
        .as_deref()
        .and_then(|source_id| self.device_detail.capture_source(source_id))
    else {
        self.session = SessionViewModel::error("No runtime capture source selected");
        return;
    };

    let Some(frame) = workspace.latest_frame.clone() else {
        self.session = SessionViewModel::starting();
        return;
    };

    self.session = SessionViewModel::streaming(source, frame);
}

impl eframe::App for HostDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let (Some(host_runtime), Some(device_id)) =
            (self.host_runtime.as_mut(), self.selected_device_id.clone())
        {
            if matches!(self.session.ui_state, SessionUiState::Streaming) {
                if let Ok(snapshot) = host_runtime.refresh_session(&device_id) {
                    self.apply_runtime_snapshot(snapshot);
                }
            }
        }
        self.sync_preview_texture(ctx);
        // existing panel rendering stays here
    }
}
```

- [ ] **Step 5: Expose real frame state in the session panel**

```rust
fn render_frame_summary(ui: &mut Ui, frame: &VideoFrameDescriptor) {
    ui.label(format!(
        "{}x{} | {}° | {:?} | frame {}",
        frame.width,
        frame.height,
        frame.rotation_degrees,
        frame.health,
        frame.frame_index
    ));
}
```

- [ ] **Step 6: Run the runtime/host/orchestrator slices to verify they pass**

Run: `cargo test -p host-desktop --test runtime_integration`
Expected: PASS

Run: `cargo test -p host-desktop --test app_state`
Expected: PASS

Run: `cargo test -p ios-control-session-orchestrator --test mock_flow`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add apps/host-desktop/src/runtime.rs \
  apps/host-desktop/src/app.rs \
  apps/host-desktop/src/view_models/session.rs \
  apps/host-desktop/src/panels/session_view.rs \
  apps/host-desktop/tests/app_state.rs \
  apps/host-desktop/tests/runtime_integration.rs \
  crates/session-orchestrator/src/lib.rs \
  crates/session-orchestrator/tests/mock_flow.rs
git commit -m "feat: drive preview state from refreshed runtime frames"
```

### Task 4: Select The Actual Control Backend And Treat Observed Success As Applied

**Files:**
- Modify: `crates/session-orchestrator/src/lib.rs`
- Modify: `crates/session-orchestrator/src/session_actor.rs`
- Modify: `crates/session-orchestrator/tests/fallback_flow.rs`
- Modify: `crates/session-orchestrator/tests/mock_flow.rs`
- Modify: `crates/session-orchestrator/tests/support/mod.rs`
- Test: `crates/session-orchestrator/tests/fallback_flow.rs`
- Test: `crates/session-orchestrator/tests/mock_flow.rs`

- [ ] **Step 1: Tighten the existing fallback test and add an observed-success regression**

```rust
#[tokio::test]
async fn supervisor_uses_window_fallback_when_ble_backend_is_unavailable() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _display_guard = prepare_window_runtime_env(&root);
    std::env::remove_var("IOS_CONTROL_BLE_HELPER");
    std::env::set_var(
        "IOS_CONTROL_WINDOW_INPUT_HELPER",
        plugin_path(&root, "plugin-control-window-bridge"),
    );

    let mut supervisor = SessionSupervisor::default();
    let status = supervisor
        .start_or_replace_session(StartSessionRequest {
            device_id: "device-ble-fallback".into(),
            device_name: "Fallback iPhone".into(),
            selected_source_id: Some("window-helper-1".into()),
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                control_ble: plugin_path(&root, "plugin-control-ble"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    assert_eq!(status.backends().control_backend, "control.window-bridge");
}

#[tokio::test]
async fn execution_result_marks_observed_change_as_applied() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _display_guard = prepare_window_runtime_env(&root);
    let helper = write_ble_helper(
        r#"{"supported":true,"supports_prepare":true,"supports_execute":true}"#,
        r#"{"phase":"Connected","checklist":["Pair the device"],"notes":[]}"#,
        r#"{"phase":"Succeeded","summary":"tap applied","observed_change":true}"#,
    );
    let _helper_guard = EnvVarGuard::set("IOS_CONTROL_BLE_HELPER", &helper);

    let mut orchestrator = SessionOrchestrator::default();
    let state = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "device-observed".into(),
            device_name: "Observed Change iPhone".into(),
            selected_source_id: Some("window-helper-1".into()),
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                control_ble: plugin_path(&root, "plugin-control-ble"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    let execution = state.execution_result.unwrap();
    assert!(execution.applied);
    assert!(execution.observed_change);
}
```

- [ ] **Step 2: Run the orchestrator control tests to verify they fail**

Run: `cargo test -p ios-control-session-orchestrator --test fallback_flow`
Expected: FAIL because the orchestrator still records `control.ble` even when BLE is unavailable and the fallback helper is configured.

Run: `cargo test -p ios-control-session-orchestrator execution_result_marks_observed_change_as_applied -- --exact`
Expected: FAIL because `ExecutionResult.applied` is still hard-coded to `false`.

- [ ] **Step 3: Add explicit control-backend selection to the orchestrator**

```rust
async fn start_control_backend(
    paths: &PluginPaths,
) -> Result<(RunningPlugin, PluginDescriptor, ControlCapability)> {
    let mut ble = RunningPlugin::spawn(&paths.control_ble).await?;
    let ble_descriptor = ble.handshake().await?;
    let ble_capability = request_control_capability(&mut ble).await?;
    if ble_capability.supported {
        return Ok((ble, ble_descriptor, ble_capability));
    }
    ble.stop().await?;

    let mut fallback = RunningPlugin::spawn(&paths.control_fallback).await?;
    let fallback_descriptor = fallback.handshake().await?;
    let fallback_capability = request_control_capability(&mut fallback).await?;
    Ok((fallback, fallback_descriptor, fallback_capability))
}
```

- [ ] **Step 4: Mark observed, successful execution as applied and carry the selected backend into status snapshots**

```rust
ExecutionDecision::ObservedChange => {
    let summary_text = format_execution_summary(&summary, true, attempts - 1);
    return Ok((
        ExecutionResult {
            applied: summary.phase == ExecutionPhase::Succeeded,
            observed_change: true,
            phase: ExecutionPhase::Succeeded,
            summary: summary_text,
            attempts,
            grounding_failure: None,
            failure_reason: None,
        },
        frame,
    ));
}

fn control_backend_for_summary(summary: &DeviceSessionSummary) -> String {
    match summary.control_plugin.as_deref() {
        Some("control.window-bridge") => "control.window-bridge".into(),
        Some("control.ble") => "control.ble".into(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}
```

- [ ] **Step 5: Run the orchestrator control slice to verify it passes**

Run: `cargo test -p ios-control-session-orchestrator --test fallback_flow`
Expected: PASS

Run: `cargo test -p ios-control-session-orchestrator --test mock_flow`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/session-orchestrator/src/lib.rs \
  crates/session-orchestrator/src/session_actor.rs \
  crates/session-orchestrator/tests/fallback_flow.rs \
  crates/session-orchestrator/tests/mock_flow.rs \
  crates/session-orchestrator/tests/support/mod.rs
git commit -m "feat: select real control backend and apply observed success"
```

### Task 5: Tighten Status-Doc Guards And Keep Validation Evidence Honest

**Files:**
- Modify: `tests/ci/test_docs_status.py`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`
- Test: `tests/ci/test_docs_status.py`

- [ ] **Step 1: Write the failing doc-honesty regression tests**

```python
def test_todo_current_reality_mentions_partial_runtime_wiring(self) -> None:
    todo = Path("docs/TODO.md").read_text(encoding="utf-8")
    self.assertIn("orchestrator-backed runtime path", todo)
    self.assertNotIn("not wired directly to the session orchestrator", todo)

def test_acceptance_matrix_only_allows_non_mock_verified_rows_with_validation_records(self) -> None:
    matrix = Path(
        "docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md"
    ).read_text(encoding="utf-8")
    for line in matrix.splitlines():
        if "| Verified |" in line and "Local mock flow" not in line:
            self.assertIn("docs/validation/", line)
```

- [ ] **Step 2: Run the docs-status suite to verify it fails**

Run: `python3 -m unittest tests.ci.test_docs_status -v`
Expected: FAIL until the stronger TODO wording assertion exists in the test file and the acceptance-matrix guard is updated to look at each verified row.

- [ ] **Step 3: Update doc guards and status wording**

```python
class DocsStatusTests(unittest.TestCase):
    def test_todo_current_reality_mentions_partial_runtime_wiring(self) -> None:
        todo = Path("docs/TODO.md").read_text(encoding="utf-8")
        self.assertIn("orchestrator-backed runtime path", todo)
        self.assertNotIn("not wired directly to the session orchestrator", todo)

    def test_acceptance_matrix_only_allows_non_mock_verified_rows_with_validation_records(self) -> None:
        matrix = Path(
            "docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md"
        ).read_text(encoding="utf-8")
        for line in matrix.splitlines():
            if "| Verified |" in line and "Local mock flow" not in line:
                self.assertIn("docs/validation/", line)
```

```markdown
- `apps/host-desktop` now has an orchestrator-backed runtime path, but the app still mixes that real runtime with legacy shell/bootstrap state and synthetic session view data.
```

- [ ] **Step 4: Run the docs-status suite to verify it passes**

Run: `python3 -m unittest tests.ci.test_docs_status -v`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add tests/ci/test_docs_status.py \
  docs/TODO.md \
  docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md
git commit -m "test: tighten status doc honesty guards"
```

### Task 6: Run The First Manual Validation Sweep And Record The Evidence

**Files:**
- Create: `docs/validation/2026-04-04-linux-window-ble.md`
- Create: `docs/validation/2026-04-04-linux-window-window-input.md`
- Create: `docs/validation/2026-04-04-windows-window-ble.md`
- Create: `docs/validation/2026-04-04-windows-window-window-input.md`
- Modify: `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`
- Modify: `docs/TODO.md`
- Test: `docs/validation/real-device-session-template.md`

- [ ] **Step 1: Build the exact binaries used in the validation sweep**

Run: `cargo build -p host-desktop -p plugin-capture-window -p plugin-control-ble -p plugin-control-window-bridge -p plugin-grounding-core`
Expected: PASS with the binaries written under `target/debug/`.

- [ ] **Step 2: Copy the template into the four dated validation-record files before running the sessions**

Run: `cp docs/validation/real-device-session-template.md docs/validation/2026-04-04-linux-window-ble.md`
Expected: PASS

Run: `cp docs/validation/real-device-session-template.md docs/validation/2026-04-04-linux-window-window-input.md`
Expected: PASS

Run: `cp docs/validation/real-device-session-template.md docs/validation/2026-04-04-windows-window-ble.md`
Expected: PASS

Run: `cp docs/validation/real-device-session-template.md docs/validation/2026-04-04-windows-window-window-input.md`
Expected: PASS

- [ ] **Step 3: Run the Linux window + BLE session and fill the Linux BLE record with the observed values**

Run: `uname -a`
Expected: PASS with the exact Linux kernel and distribution string you will paste into `docs/validation/2026-04-04-linux-window-ble.md` under `Host OS`.

Run: `bluetoothctl list`
Expected: PASS with the adapter name and address you will paste into `docs/validation/2026-04-04-linux-window-ble.md` under `Host Bluetooth adapter`.

Run: `cargo run -p host-desktop`
Expected: The `iOS Control Host` window opens and, after configuring the BLE helper and pairing the device, the control diagnostics move out of `Unavailable`.

Replace the copied bullets in `docs/validation/2026-04-04-linux-window-ble.md` with:
- the exact `uname -a` output
- the exact `bluetoothctl list` adapter line
- the exact iPhone/iPad model name from `Settings > General > About`
- the observed pairing, live preview, live control, and recovery outcomes from this run
- the exact mirror app/helper path and any latency or failure notes

- [ ] **Step 4: Run the Linux window + window-input fallback session and fill the Linux fallback record**

Run: `uname -a`
Expected: PASS with the exact Linux kernel and distribution string you will paste into `docs/validation/2026-04-04-linux-window-window-input.md` under `Host OS`.

Run: `IOS_CONTROL_BLE_HELPER= IOS_CONTROL_WINDOW_INPUT_HELPER=target/debug/plugin-control-window-bridge cargo run -p host-desktop`
Expected: The host starts with the window-input fallback path selected when BLE helper support is absent.

Replace the copied bullets in `docs/validation/2026-04-04-linux-window-window-input.md` with:
- the exact `uname -a` output
- `N/A` for `Host Bluetooth adapter`
- `window helper` for `Capture path`
- `window input bridge` for `Control path`
- the exact iPhone/iPad model name from `Settings > General > About`
- `N/A` for `Pairing result`
- the observed live preview, live control, and recovery outcomes from this run
- the exact mirrored-window app name plus focus/visibility requirements

- [ ] **Step 5: Repeat the same procedure on Windows and update the acceptance matrix only for the rows that now have matching dated records**

Run: `powershell -Command "Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version"`
Expected: PASS with the exact Windows name and version you will paste into the Windows validation records under `Host OS`.

Run: `powershell -Command "Get-PnpDevice -Class Bluetooth | Select-Object FriendlyName, Status"`
Expected: PASS with the Bluetooth adapter row you will paste into `docs/validation/2026-04-04-windows-window-ble.md` under `Host Bluetooth adapter`.

After the Windows BLE and Windows fallback runs complete, replace the bullets in:
- `docs/validation/2026-04-04-windows-window-ble.md`
- `docs/validation/2026-04-04-windows-window-window-input.md`

Use the exact command output above, the exact device model name from `Settings > General > About`, and the exact observed pairing/live-preview/live-control/recovery outcomes from the two Windows runs.

```markdown
| Linux multi-device | Window helper | BLE HID | Verified manually | Verified manually | Verified manually | Verified manually | Verified ([docs/validation/2026-04-04-linux-window-ble.md](docs/validation/2026-04-04-linux-window-ble.md)) |
| Linux fallback | Window helper | Window input bridge | N/A | Verified manually | Verified manually | Verified manually | Verified ([docs/validation/2026-04-04-linux-window-window-input.md](docs/validation/2026-04-04-linux-window-window-input.md)) |
```

- [ ] **Step 6: Run the docs-status suite one more time after recording any verified runs**

Run: `python3 -m unittest tests.ci.test_docs_status -v`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add docs/validation/2026-04-04-linux-window-ble.md \
  docs/validation/2026-04-04-linux-window-window-input.md \
  docs/validation/2026-04-04-windows-window-ble.md \
  docs/validation/2026-04-04-windows-window-window-input.md \
  docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md \
  docs/TODO.md
git commit -m "docs: record first real-device validation sweep"
```
