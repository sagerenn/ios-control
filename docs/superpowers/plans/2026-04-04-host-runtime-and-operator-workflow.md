# Host Runtime And Operator Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the synthetic host runtime with an orchestrator-backed host runtime and make the desktop UI start, stop, and inspect real local sessions.

**Architecture:** Keep `apps/host-desktop` as the egui shell, but replace `HostRuntimeBridge` with a real `HostRuntime` wrapper that owns a local Tokio runtime and a `SessionSupervisor`. Feed the host app with a richer runtime snapshot that includes fleet rows, per-device workspace state, capture sources, diagnostics, and control checklist data, so the UI stops inventing state on its own.

**Tech Stack:** Rust, tokio, eframe/egui, `ios-control-session-orchestrator`, plugin-backed local runtime, Cargo integration tests

---

## File Structure

- `apps/host-desktop/Cargo.toml`: add orchestrator and Tokio dependencies for the host runtime wrapper.
- `apps/host-desktop/src/runtime.rs`: replace the in-memory bridge with an orchestrator-backed runtime facade.
- `apps/host-desktop/src/app.rs`: start and stop sessions through the real runtime, and sync UI state from runtime snapshots.
- `apps/host-desktop/src/panels/device_detail.rs`: return operator actions for capture-source selection.
- `apps/host-desktop/src/main.rs`: construct the host app with runtime configuration.
- `apps/host-desktop/tests/support/mod.rs`: shared plugin-build and helper-env setup for host tests.
- `apps/host-desktop/tests/runtime_integration.rs`: regression coverage for runtime start, stop, and snapshot behavior.
- `apps/host-desktop/tests/app_state.rs`: app-level coverage for runtime-backed session start and operator workflow.
- `crates/session-orchestrator/src/lib.rs`: add explicit stop APIs and snapshot helpers needed by the host runtime.

### Task 1: Add A Real Host Runtime Wrapper

**Files:**
- Modify: `apps/host-desktop/Cargo.toml`
- Modify: `apps/host-desktop/src/runtime.rs`
- Modify: `crates/session-orchestrator/src/lib.rs`
- Create: `apps/host-desktop/tests/support/mod.rs`
- Create: `apps/host-desktop/tests/runtime_integration.rs`
- Test: `apps/host-desktop/tests/runtime_integration.rs`

- [ ] **Step 1: Write the failing runtime integration test**

```rust
use host_desktop::runtime::{HostRuntime, HostRuntimeConfig};
use ios_control_contracts::session::SessionPhase;

mod support;
use support::{build_plugins, host_plugin_paths, prepare_window_runtime_env, runtime_env_lock, workspace_root};

#[test]
fn runtime_start_session_returns_workspace_snapshot() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);
    let _guards = prepare_window_runtime_env(&root);

    let mut runtime = HostRuntime::new(HostRuntimeConfig {
        plugin_paths: host_plugin_paths(&root),
    })
    .unwrap();

    let snapshot = runtime
        .start_session("device-1", "Mock iPhone", Some("window-helper-1".into()))
        .unwrap();

    assert_eq!(snapshot.statuses.len(), 1);
    assert_eq!(snapshot.workspace.device_id, "device-1");
    assert!(matches!(
        snapshot.workspace.summary.phase,
        SessionPhase::Streaming | SessionPhase::Degraded
    ));
    assert_eq!(snapshot.workspace.capture_sources.len(), 1);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p host-desktop runtime_start_session_returns_workspace_snapshot -- --exact`

Expected: FAIL with unresolved imports for `HostRuntime`, `HostRuntimeConfig`, or missing runtime support methods.

- [ ] **Step 3: Write the minimal runtime wrapper and supervisor stop API**

```rust
use anyhow::Result;
use ios_control_session_orchestrator::{PluginPaths, SessionSupervisor, StartSessionRequest};

#[derive(Debug, Clone)]
pub struct HostRuntimeConfig {
    pub plugin_paths: PluginPaths,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWorkspaceState {
    pub device_id: String,
    pub summary: ios_control_contracts::session::DeviceSessionSummary,
    pub capture_sources: Vec<ios_control_contracts::capture::VideoSource>,
    pub selected_source_id: Option<String>,
    pub control_checklist: ios_control_contracts::control::ControlSetupChecklist,
    pub diagnostics: ios_control_session_orchestrator::SessionDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRuntimeSnapshot {
    pub statuses: Vec<ios_control_contracts::session::DeviceSessionStatus>,
    pub workspace: RuntimeWorkspaceState,
}

pub struct HostRuntime {
    tokio: tokio::runtime::Runtime,
    supervisor: SessionSupervisor,
    config: HostRuntimeConfig,
}

impl HostRuntime {
    pub fn new(config: HostRuntimeConfig) -> Result<Self> {
        Ok(Self {
            tokio: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?,
            supervisor: SessionSupervisor::default(),
            config,
        })
    }

    pub fn start_session(
        &mut self,
        device_id: &str,
        device_name: &str,
        selected_source_id: Option<String>,
    ) -> Result<HostRuntimeSnapshot> {
        self.tokio.block_on(self.supervisor.start_or_replace_session(StartSessionRequest {
            device_id: device_id.into(),
            device_name: device_name.into(),
            selected_source_id,
            plugin_paths: self.config.plugin_paths.clone(),
        }))?;
        self.snapshot(device_id)
    }

    pub fn snapshot(&self, device_id: &str) -> Result<HostRuntimeSnapshot> {
        let status = self
            .supervisor
            .session_statuses()
            .get(device_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing runtime status for {device_id}"))?;
        let active = self
            .supervisor
            .active_sessions()
            .get(device_id)
            .ok_or_else(|| anyhow::anyhow!("missing active session for {device_id}"))?;
        Ok(HostRuntimeSnapshot {
            statuses: self.supervisor.session_statuses().values().cloned().collect(),
            workspace: RuntimeWorkspaceState {
                device_id: device_id.into(),
                summary: status.summary().clone(),
                capture_sources: active.capture_sources.clone(),
                selected_source_id: active.selected_source_id.clone(),
                control_checklist: active.control_checklist.clone(),
                diagnostics: active.diagnostics.clone(),
            },
        })
    }
}

impl SessionSupervisor {
    pub async fn stop_session(&mut self, device_id: &str) -> Result<()> {
        if let Some(active) = self.active.remove(device_id) {
            active.shutdown().await?;
        }
        self.sessions.remove(device_id);
        Ok(())
    }
}
```

- [ ] **Step 4: Run the runtime integration test to verify it passes**

Run: `cargo test -p host-desktop runtime_start_session_returns_workspace_snapshot -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/host-desktop/Cargo.toml \
  apps/host-desktop/src/runtime.rs \
  apps/host-desktop/tests/support/mod.rs \
  apps/host-desktop/tests/runtime_integration.rs \
  crates/session-orchestrator/src/lib.rs
git commit -m "feat: add orchestrator-backed host runtime"
```

### Task 2: Replace The Bootstrap Error Path With Real Runtime Start And Stop

**Files:**
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/src/main.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`
- Test: `apps/host-desktop/tests/app_state.rs`

- [ ] **Step 1: Write the failing app-state tests**

```rust
#[test]
fn host_app_start_session_uses_real_runtime_snapshot() {
    let mut app = support::host_app_with_runtime();
    app.select_device("device-1");

    app.request_start_session();

    assert!(matches!(app.session.ui_state, SessionUiState::Streaming | SessionUiState::Error(_)));
    assert_ne!(
        app.diagnostics.host_error.as_deref(),
        Some("Session bootstrap is not wired to the runtime yet")
    );
}

#[test]
fn host_app_stop_session_removes_runtime_status() {
    let mut app = support::host_app_with_runtime();
    app.select_device("device-1");
    app.request_start_session();

    app.stop_session();

    assert!(app.available_device_ids.is_empty());
    assert_eq!(app.session.ui_state, SessionUiState::Idle);
}
```

- [ ] **Step 2: Run the app-state tests to verify they fail**

Run: `cargo test -p host-desktop host_app_start_session_uses_real_runtime_snapshot -- --exact`

Expected: FAIL because `request_start_session()` still drives the synthetic bootstrap path.

- [ ] **Step 3: Update the app to call the runtime instead of simulating startup**

```rust
pub fn request_start_session(&mut self) {
    let Some(device_id) = self
        .selected_device_id
        .clone()
        .or_else(|| self.available_device_ids.first().cloned())
    else {
        self.session = SessionViewModel::error("No device selected");
        return;
    };

    self.session = SessionViewModel::starting();
    self.diagnostics.host_error = None;

    match self.runtime.start_session(
        &device_id,
        &self.device_detail.device_name,
        self.device_detail.active_source_id.clone(),
    ) {
        Ok(snapshot) => self.apply_runtime_snapshot(snapshot),
        Err(error) => {
            let message = error.to_string();
            self.session = SessionViewModel::error(&message);
            self.diagnostics.host_error = Some(message);
        }
    }
}

pub fn stop_session(&mut self) {
    if let Some(device_id) = self.selected_device_id.clone() {
        let _ = self.runtime.stop_session(&device_id);
    }
    self.session = SessionViewModel::idle();
    self.device_detail.active_source_id = None;
    self.diagnostics.host_error = None;
    self.diagnostics.control_summary = "control not started".into();
    self.diagnostics.grounding_summary = "grounding idle".into();
}
```

- [ ] **Step 4: Run the app-state test file to verify the runtime-backed path passes**

Run: `cargo test -p host-desktop --test app_state`

Expected: PASS with no assertion depending on the old bootstrap-error string.

- [ ] **Step 5: Commit**

```bash
git add apps/host-desktop/src/app.rs \
  apps/host-desktop/src/main.rs \
  apps/host-desktop/tests/app_state.rs
git commit -m "feat: wire host app session actions to runtime"
```

### Task 3: Finish Operator Workflow State Sync

**Files:**
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/src/panels/device_detail.rs`
- Modify: `apps/host-desktop/src/view_models/device_detail.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`
- Test: `apps/host-desktop/tests/app_state.rs`

- [ ] **Step 1: Write the failing workflow tests**

```rust
#[test]
fn selecting_a_capture_source_updates_runtime_selection() {
    let mut app = support::host_app_with_runtime();
    app.request_start_session();

    app.select_capture_source("window-helper-1");

    assert_eq!(app.device_detail.active_source_id.as_deref(), Some("window-helper-1"));
}

#[test]
fn runtime_snapshot_populates_control_checklist_and_operator_message() {
    let mut app = support::host_app_with_runtime();
    app.request_start_session();

    assert!(!app.device_detail.control_checklist.items.is_empty());
    assert!(app.diagnostics.control_summary.contains("control"));
}
```

- [ ] **Step 2: Run the workflow tests to verify they fail**

Run: `cargo test -p host-desktop selecting_a_capture_source_updates_runtime_selection -- --exact`

Expected: FAIL because the UI has no capture-source action path and only mirrors synthetic values.

- [ ] **Step 3: Add runtime snapshot application and device-detail actions**

```rust
pub enum DeviceDetailAction {
    None,
    SelectCaptureSource(String),
}

pub fn render(
    ui: &mut Ui,
    view_model: &DeviceDetailViewModel,
) -> DeviceDetailAction {
    let mut action = DeviceDetailAction::None;
    ui.heading("Device Detail");
    ui.label(&view_model.device_name);
    for source in &view_model.capture_sources {
        let selected = view_model.active_source_id.as_deref() == Some(source.source_id.as_str());
        if ui.selectable_label(selected, source.label()).clicked() {
            action = DeviceDetailAction::SelectCaptureSource(source.source_id.clone());
        }
    }
    for item in &view_model.control_checklist.items {
        ui.label(item);
    }
    action
}

fn apply_runtime_snapshot(&mut self, snapshot: HostRuntimeSnapshot) {
    self.available_device_ids = snapshot
        .statuses
        .iter()
        .map(|status| status.summary().device_id.clone())
        .collect();
    self.device_detail.active_source_id = snapshot.workspace.selected_source_id;
    self.device_detail.control_checklist.items = snapshot.workspace.control_checklist.items;
    self.diagnostics.control_summary = snapshot.workspace.diagnostics.control_summary;
}
```

- [ ] **Step 4: Run the host-desktop test suite to verify the operator workflow passes**

Run: `cargo test -p host-desktop`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/host-desktop/src/app.rs \
  apps/host-desktop/src/panels/device_detail.rs \
  apps/host-desktop/src/view_models/device_detail.rs \
  apps/host-desktop/tests/app_state.rs
git commit -m "feat: sync operator workflow from host runtime snapshots"
```
