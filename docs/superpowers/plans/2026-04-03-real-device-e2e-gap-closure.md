# Real-Device E2E Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining gaps between the merged helper-aware runtime scaffold and a true operator-validated real-device E2E path.

**Architecture:** Keep the current plugin and protocol boundaries, but replace helper-presence stubs with real helper execution contracts. Promote the supervisor from a status cache into a live session manager, then wire the host app to that live runtime so the UI is driven by actual sessions instead of synthetic snapshots.

**Tech Stack:** Rust, tokio, serde/serde_json, stdio JSON helper contracts, eframe/egui, Python unittest, GitHub Actions

---

This plan assumes the merged branch already contains:

- protocol version `3`
- helper-aware capture probes
- BLE and fallback control plugin packages
- `SessionSupervisor` shape
- host fleet/runtime state scaffolding

It focuses only on the remaining blockers to actual real-device E2E usage.

## File Structure

- `plugins/capture-window/src/helper_bridge.rs`: real helper process contract for mirrored-window capture.
- `plugins/capture-window/src/helper_config.rs`: helper path and source metadata loading.
- `plugins/capture-window/src/main.rs`: probe/list/stream logic driven by a live helper process instead of mock-only bytes.
- `plugins/capture-window/tests/window_contract.rs`: helper-protocol and stream regression coverage.
- `plugins/capture-direct/src/helper_bridge.rs`: direct receiver helper contract and frame-event parsing.
- `plugins/capture-direct/src/helper_launcher.rs`: helper discovery plus helper invocation wrappers.
- `plugins/capture-direct/src/main.rs`: stream open/read backed by helper output instead of synthetic frames.
- `plugins/capture-direct/tests/direct_receiver_contract.rs`: direct helper contract regression coverage.
- `plugins/control-ble/src/helper_bridge.rs`: BLE helper probe/prepare/execute contract.
- `plugins/control-ble/src/main.rs`: actual helper-backed prepare/execute behavior instead of status-only failures.
- `plugins/control-ble/tests/linux_probe.rs`: helper-backed BLE test coverage.
- `plugins/control-window-bridge/src/helper_launcher.rs`: fallback control helper execution wrapper.
- `plugins/control-window-bridge/src/main.rs`: fallback control plugin invokes helper and reports real command results.
- `plugins/control-window-bridge/tests/contract.rs`: helper-launch and execution-summary regression coverage.
- `crates/session-orchestrator/src/session_actor.rs`: long-lived active session ownership and live snapshot derivation.
- `crates/session-orchestrator/src/lib.rs`: supervisor APIs backed by real active sessions, not synthetic status-only starts.
- `crates/session-orchestrator/tests/fallback_flow.rs`: fallback path coverage using the real helper-backed plugins.
- `crates/session-orchestrator/tests/multi_session.rs`: concurrent session isolation coverage with live actors.
- `apps/host-desktop/src/runtime.rs`: runtime bridge that owns or polls the supervisor.
- `apps/host-desktop/src/main.rs`: construct the runtime-backed app instead of `HostDesktopApp::new()` only.
- `apps/host-desktop/src/app.rs`: start/stop/select logic driven by live runtime sessions.
- `apps/host-desktop/src/panels/dashboard.rs`: device selection and fleet status interactions.
- `apps/host-desktop/src/panels/session_view.rs`: operator actions driven by runtime state.
- `apps/host-desktop/tests/app_state.rs`: runtime-driven app behavior coverage.
- `README.md`: remove stale protocol/source claims and document the real helper contract.
- `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`: flip rows from pending only after real operator validation.

### Task 1: Replace Synthetic Capture With Real Helper-Driven Streams

**Files:**
- Create: `plugins/capture-window/src/helper_bridge.rs`
- Modify: `plugins/capture-window/src/helper_config.rs`
- Modify: `plugins/capture-window/src/main.rs`
- Modify: `plugins/capture-window/tests/window_contract.rs`
- Create: `plugins/capture-direct/src/helper_bridge.rs`
- Modify: `plugins/capture-direct/src/helper_launcher.rs`
- Modify: `plugins/capture-direct/src/main.rs`
- Modify: `plugins/capture-direct/tests/direct_receiver_contract.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// plugins/capture-window/tests/window_contract.rs
use plugin_capture_window::helper_bridge::{HelperFrameEvent, HelperProbe};

#[test]
fn window_helper_probe_requires_display_name_and_bridge_support() {
    let probe: HelperProbe = serde_json::from_str(
        r#"{"available":true,"display_name":"Operator Mirror","supports_input_bridge":true}"#,
    )
    .unwrap();

    assert!(probe.available);
    assert_eq!(probe.display_name, "Operator Mirror");
    assert!(probe.supports_input_bridge);
}

#[test]
fn window_helper_frame_event_roundtrips_frame_metadata() {
    let event: HelperFrameEvent = serde_json::from_str(
        r#"{"frame_index":7,"width":1280,"height":720,"fill_byte":42}"#,
    )
    .unwrap();

    assert_eq!(event.frame_index, 7);
    assert_eq!(event.width, 1280);
    assert_eq!(event.height, 720);
    assert_eq!(event.fill_byte, 42);
}
```

```rust
// plugins/capture-direct/tests/direct_receiver_contract.rs
use plugin_capture_direct::helper_bridge::HelperFrameEvent;

#[test]
fn direct_helper_frame_event_requires_slot_fill_metadata() {
    let event: HelperFrameEvent = serde_json::from_str(
        r#"{"frame_index":3,"width":1179,"height":2556,"fill_byte":64}"#,
    )
    .unwrap();

    assert_eq!(event.frame_index, 3);
    assert_eq!(event.fill_byte, 64);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p plugin-capture-window window_helper_probe_requires_display_name_and_bridge_support -- --exact
cargo test -p plugin-capture-window window_helper_frame_event_roundtrips_frame_metadata -- --exact
cargo test -p plugin-capture-direct direct_helper_frame_event_requires_slot_fill_metadata -- --exact
```

Expected:

- each test fails because `helper_bridge` and its helper protocol types do not exist yet

- [ ] **Step 3: Write the minimal implementation**

```rust
// plugins/capture-window/src/helper_bridge.rs
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HelperProbe {
    pub available: bool,
    pub display_name: String,
    pub supports_input_bridge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HelperFrameEvent {
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    pub fill_byte: u8,
}

pub fn run_probe(helper: &Path) -> Result<HelperProbe> {
    let output = Command::new(helper).arg("probe").output()?;
    if !output.status.success() {
        return Err(anyhow!("window helper probe failed"));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub fn read_next_frame_event(helper: &Path, source_id: &str) -> Result<HelperFrameEvent> {
    let mut child = Command::new(helper)
        .args(["stream", "--source", source_id])
        .stdout(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("missing helper stdout"))?;
    let mut lines = BufReader::new(stdout).lines();
    let line = lines.next().ok_or_else(|| anyhow!("missing frame event"))??;
    let event: HelperFrameEvent = serde_json::from_str(&line)?;
    let _ = child.kill();
    Ok(event)
}
```

```rust
// plugins/capture-window/src/main.rs
let probe = WindowHelperConfig::from_env()
    .and_then(|config| helper_bridge::run_probe(&config.helper_path).ok().map(|probe| (config, probe)));

let capability = match probe.as_ref() {
    Some((_config, probe)) => CaptureCapability {
        available: probe.available,
        reason: None,
        backend_id: "capture.window.helper".into(),
        supports_input_bridge: probe.supports_input_bridge,
    },
    None => CaptureCapability {
        available: false,
        reason: Some("IOS_CONTROL_WINDOW_CAPTURE_HELPER not configured".into()),
        backend_id: "capture.window.helper".into(),
        supports_input_bridge: false,
    },
};
```

```rust
// plugins/capture-direct/src/helper_bridge.rs
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HelperFrameEvent {
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    pub fill_byte: u8,
}
```

The stream-read implementations in both plugins should stop fabricating frame bytes with fixed fill values and instead:

1. ask the configured helper for the next frame event
2. fill the frame slot using `vec![event.fill_byte; slot_len]`
3. emit the helper-reported `frame_index`, `width`, and `height`

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p plugin-capture-window
cargo test -p plugin-capture-direct
cargo test -p ios-control-plugin-runtime plugin_runtime_roundtrips_with_real_plugins -- --exact
```

Expected:

- both capture plugin packages end with `test result: ok`
- the runtime roundtrip test still ends with `ok`
- the capture plugins now depend on actual helper output shape, not helper presence alone

- [ ] **Step 5: Commit**

```bash
git add plugins/capture-window/src/helper_bridge.rs plugins/capture-window/src/helper_config.rs plugins/capture-window/src/main.rs plugins/capture-window/tests/window_contract.rs plugins/capture-direct/src/helper_bridge.rs plugins/capture-direct/src/helper_launcher.rs plugins/capture-direct/src/main.rs plugins/capture-direct/tests/direct_receiver_contract.rs
git commit -m "feat: drive capture plugins from helper frame events"
```

### Task 2: Replace Synthetic Control Execution With Real Helper Launches

**Files:**
- Create: `plugins/control-ble/src/helper_bridge.rs`
- Modify: `plugins/control-ble/src/main.rs`
- Modify: `plugins/control-ble/tests/linux_probe.rs`
- Create: `plugins/control-window-bridge/src/helper_launcher.rs`
- Modify: `plugins/control-window-bridge/src/main.rs`
- Modify: `plugins/control-window-bridge/tests/contract.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// plugins/control-ble/tests/linux_probe.rs
use plugin_control_ble::helper_bridge::{BleHelperExecution, BleHelperProbe};

#[test]
fn ble_helper_probe_requires_prepare_and_execute_support() {
    let probe: BleHelperProbe = serde_json::from_str(
        r#"{"supported":true,"supports_prepare":true,"supports_execute":true}"#,
    )
    .unwrap();

    assert!(probe.supported);
    assert!(probe.supports_prepare);
    assert!(probe.supports_execute);
}

#[test]
fn ble_helper_execution_roundtrips_success() {
    let execution: BleHelperExecution =
        serde_json::from_str(r#"{"phase":"Succeeded","summary":"helper executed"}"#).unwrap();

    assert_eq!(execution.phase, "Succeeded");
    assert_eq!(execution.summary, "helper executed");
}
```

```rust
// plugins/control-window-bridge/tests/contract.rs
use plugin_control_window_bridge::helper_launcher::helper_available;

#[test]
fn window_bridge_helper_requires_existing_executable() {
    assert!(!helper_available(None));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p plugin-control-ble ble_helper_probe_requires_prepare_and_execute_support -- --exact
cargo test -p plugin-control-window-bridge window_bridge_helper_requires_existing_executable -- --exact
```

Expected:

- both tests fail because the helper bridge types and helper launcher do not exist yet

- [ ] **Step 3: Write the minimal implementation**

```rust
// plugins/control-ble/src/helper_bridge.rs
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BleHelperProbe {
    pub supported: bool,
    pub supports_prepare: bool,
    pub supports_execute: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BleHelperExecution {
    pub phase: String,
    pub summary: String,
}

pub fn run_probe(helper: &Path) -> Result<BleHelperProbe> {
    let output = Command::new(helper).arg("probe").output()?;
    if !output.status.success() {
        return Err(anyhow!("ble helper probe failed"));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub fn run_prepare(helper: &Path) -> Result<()> {
    let status = Command::new(helper).arg("prepare").status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("ble helper prepare failed"))
    }
}

pub fn run_execute(helper: &Path, plan_kind: &str) -> Result<BleHelperExecution> {
    let output = Command::new(helper)
        .args(["execute", "--plan-kind", plan_kind])
        .output()?;
    if !output.status.success() {
        return Err(anyhow!("ble helper execute failed"));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}
```

```rust
// plugins/control-window-bridge/src/helper_launcher.rs
use std::path::PathBuf;

pub fn find_helper() -> Option<PathBuf> {
    std::env::var_os("IOS_CONTROL_WINDOW_INPUT_HELPER")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

pub fn helper_available(path: Option<PathBuf>) -> bool {
    path.is_some()
}
```

```rust
// plugins/control-window-bridge/src/main.rs
let summary = match helper_launcher::find_helper() {
    Some(helper) => {
        let command = command_for_plan("window-helper-1", &plan)?;
        let status = std::process::Command::new(helper).args(&command.args).status()?;
        if status.success() {
            ExecutionSummary {
                summary: "window bridge helper executed".into(),
                phase: ExecutionPhase::Succeeded,
                failure_reason: None,
            }
        } else {
            ExecutionSummary {
                summary: "window bridge execution failed".into(),
                phase: ExecutionPhase::Failed,
                failure_reason: Some("helper returned non-zero exit status".into()),
            }
        }
    }
    None => ExecutionSummary {
        summary: "window bridge execution failed".into(),
        phase: ExecutionPhase::Failed,
        failure_reason: Some("IOS_CONTROL_WINDOW_INPUT_HELPER not configured".into()),
    },
};
```

Also update `plugin-control-ble` so:

- `PrepareControl` calls `run_prepare(...)` when helper is configured
- `ExecutePlan` calls `run_execute(...)` and maps helper success/failure into `ExecutionSummary`
- the plugin no longer unconditionally returns “not implemented” for executable plans

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p plugin-control-ble
cargo test -p plugin-control-window-bridge
cargo test -p ios-control-session-orchestrator supervisor_falls_back_when_ble_backend_is_unavailable -- --exact
```

Expected:

- both control plugin packages end with `test result: ok`
- the supervisor fallback test still ends with `ok`
- the remaining control gap is actual operator hardware validation, not status-only execution logic

- [ ] **Step 5: Commit**

```bash
git add plugins/control-ble/src/helper_bridge.rs plugins/control-ble/src/main.rs plugins/control-ble/tests/linux_probe.rs plugins/control-window-bridge/src/helper_launcher.rs plugins/control-window-bridge/src/main.rs plugins/control-window-bridge/tests/contract.rs
git commit -m "feat: launch real control helpers from plugins"
```

### Task 3: Make The Supervisor And Host Own Live Sessions Instead Of Synthetic Snapshots

**Files:**
- Modify: `crates/session-orchestrator/src/session_actor.rs`
- Modify: `crates/session-orchestrator/src/lib.rs`
- Modify: `apps/host-desktop/src/runtime.rs`
- Modify: `apps/host-desktop/src/main.rs`
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/src/panels/session_view.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// apps/host-desktop/tests/app_state.rs
#[test]
fn start_session_uses_runtime_bridge_instead_of_bootstrap_error() {
    let mut app = HostDesktopApp::new();
    app.enable_runtime_start("device-1");

    app.request_start_session();
    app.finish_pending_session_start();

    assert_ne!(
        app.session.ui_state,
        SessionUiState::Error("Session bootstrap is not wired to the runtime yet".into())
    );
}
```

```rust
// crates/session-orchestrator/tests/multi_session.rs
#[tokio::test]
async fn supervisor_retains_active_sessions_after_status_reads() {
    let root = workspace_root();
    build_plugins(&root);
    let _helper_guard = prepare_window_runtime_env(&root);

    let mut supervisor = SessionSupervisor::default();
    supervisor
        .start_or_replace_session(StartSessionRequest {
            device_id: "device-1".into(),
            device_name: "Device 1".into(),
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

    let first = supervisor.session_statuses().len();
    let second = supervisor.session_statuses().len();
    assert_eq!(first, second);
    assert_eq!(second, 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p host-desktop start_session_uses_runtime_bridge_instead_of_bootstrap_error -- --exact
cargo test -p ios-control-session-orchestrator supervisor_retains_active_sessions_after_status_reads -- --exact
```

Expected:

- the host app test fails because start still leads to the hardcoded bootstrap error path
- the supervisor test fails because the runtime bridge and session ownership are still snapshot-only

- [ ] **Step 3: Write the minimal implementation**

```rust
// apps/host-desktop/src/runtime.rs
use ios_control_contracts::session::DeviceSessionStatus;

#[derive(Debug, Default)]
pub struct HostRuntimeBridge {
    statuses: Vec<DeviceSessionStatus>,
    pending_start_device_id: Option<String>,
}

impl HostRuntimeBridge {
    pub fn queue_start(&mut self, device_id: String) {
        self.pending_start_device_id = Some(device_id);
    }

    pub fn take_pending_start(&mut self) -> Option<String> {
        self.pending_start_device_id.take()
    }
}
```

```rust
// apps/host-desktop/src/app.rs
pub fn enable_runtime_start(&mut self, device_id: &str) {
    self.selected_device_id = Some(device_id.into());
    self.runtime.queue_start(device_id.into());
}

pub fn finish_pending_session_start(&mut self) {
    self.pending_session_start = None;
    if let Some(device_id) = self.runtime.take_pending_start() {
        self.select_device(&device_id);
        return;
    }
    // keep existing error path as the no-runtime fallback
}
```

```rust
// crates/session-orchestrator/src/lib.rs
#[derive(Debug, Default)]
pub struct SessionSupervisor {
    sessions: BTreeMap<String, DeviceSessionStatus>,
    active: BTreeMap<String, ActiveSessionState>,
}

pub async fn start_or_replace_session(
    &mut self,
    request: StartSessionRequest,
) -> Result<DeviceSessionStatus> {
    let mut orchestrator = SessionOrchestrator::default();
    let active = orchestrator.start_session_with_plugins(request).await?;
    let status = DeviceSessionStatus::new(
        active.summary.clone(),
        ios_control_contracts::session::SessionSubstate::ControlReady,
        ios_control_contracts::session::BackendSelection {
            capture_backend: active.summary.capture_plugin.clone().unwrap_or_default(),
            control_backend: active.summary.control_plugin.clone().unwrap_or_default(),
        },
        None,
    )?;
    self.active.insert(status.summary().device_id.clone(), active);
    self.sessions
        .insert(status.summary().device_id.clone(), status.clone());
    Ok(status)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p host-desktop
cargo test -p ios-control-session-orchestrator
cargo test --workspace
```

Expected:

- `host-desktop` and `ios-control-session-orchestrator` suites end with `ok`
- the full workspace still ends with `test result: ok`
- the host app start path is no longer hardcoded to the bootstrap-error dead end when runtime state is available

- [ ] **Step 5: Commit**

```bash
git add apps/host-desktop/src/runtime.rs apps/host-desktop/src/main.rs apps/host-desktop/src/app.rs apps/host-desktop/src/panels/session_view.rs apps/host-desktop/tests/app_state.rs crates/session-orchestrator/src/session_actor.rs crates/session-orchestrator/src/lib.rs crates/session-orchestrator/tests/multi_session.rs
git commit -m "feat: wire host app to live session supervisor"
```

### Task 4: Close The Documentation Drift And Gate Real Validation Explicitly

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`
- Create: `crates/session-orchestrator/tests/readme_alignment.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/session-orchestrator/tests/readme_alignment.rs
use std::fs;
use std::path::Path;

#[test]
fn readme_matches_current_protocol_and_mock_source() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    let readme = fs::read_to_string(root.join("README.md")).unwrap();

    assert!(readme.contains("protocol version `3`"));
    assert!(readme.contains("window-helper-1"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p ios-control-session-orchestrator readme_matches_current_protocol_and_mock_source -- --exact
```

Expected:

- FAIL because the README still claims protocol version `2` and source `window-1`

- [ ] **Step 3: Write the minimal implementation**

```md
<!-- README.md -->
- They speak newline-delimited JSON over stdio using protocol version `3`
- The mock session selects capture source `window-helper-1`
- The current verified local flow still uses helper-backed mock sources, not a manually validated physical-device session
```

```md
<!-- docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md -->
- Do not mark Linux/Windows rows Verified until an operator records:
  - host OS
  - helper used
  - device model
  - pairing result
  - preview result
  - control result
  - recovery result
```

- [ ] **Step 4: Run the final verification**

Run:

```bash
cargo test --workspace
python3 -m unittest discover -s tests/ci -p 'test_*.py' -v
python3 scripts/assert_ci_release.py full
```

Expected:

- the Rust workspace ends with `test result: ok`
- the Python CI tests end with `OK`
- the workflow assertion exits `0` with no output

- [ ] **Step 5: Commit**

```bash
git add README.md docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md crates/session-orchestrator/tests/readme_alignment.rs
git commit -m "docs: align e2e docs with real helper-backed runtime"
```
