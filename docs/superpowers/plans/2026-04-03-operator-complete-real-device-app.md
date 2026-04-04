# Operator-Complete Real-Device App Implementation Plan

> Historical planning artifact. This plan captures an intended implementation path from 2026-04-03. It should not be read as proof that the current branch is operator-complete. For current status, use `README.md`, `docs/TODO.md`, and `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the current mock-oriented workspace into an operator-grade app that can supervise multiple concurrent real-device sessions on Linux and Windows, using real capture backends, BLE-preferred control, explicit fallback control, and visible diagnostics/recovery.

**Architecture:** Define the shared contract layer first so capture, control, runtime, and host UI can evolve against one stable session model. After that lands, implement helper-backed capture, helper-backed BLE plus mirrored-window fallback control, a multi-session supervisor in the orchestrator, and a real operator console in the host app. Finish by updating packaging, CI expectations, and operator documentation to match the actual shipped product.

**Tech Stack:** Rust, Cargo workspace, tokio, serde/serde_json, anyhow, memmap2, eframe/egui, newline-delimited JSON stdio IPC, Python CI/package tests, helper-backed runtime integrations

---

This plan is intentionally split into one critical-path contract task and four parallelizable product workstreams. After Task 1 lands, Tasks 2-5 can be implemented in parallel with disjoint ownership. Task 6 closes packaging, docs, and validation.

## File Structure

- `Cargo.toml`: workspace membership for the new fallback-control plugin.
- `crates/contracts/src/capture.rs`: capture capability and helper bridge support metadata.
- `crates/contracts/src/control.rs`: control transport identity and normalized capability reporting.
- `crates/contracts/src/session.rs`: operator-facing sub-state, backend selection, and multi-session status snapshots.
- `crates/contracts/tests/session_contract.rs`: session snapshot regression coverage.
- `crates/plugin-protocol/src/lib.rs`: capture probe messages and normalized capability replies.
- `crates/plugin-protocol/tests/operations_roundtrip.rs`: protocol roundtrip coverage for new capability messages.
- `crates/plugin-runtime/tests/plugin_roundtrip.rs`: runtime handshake and capture-probe coverage for the updated plugins.
- `plugins/capture-window/src/helper_config.rs`: operator-configured mirrored-window helper discovery.
- `plugins/capture-window/src/linux_backend.rs`: Linux capture capability detection backed by helper config.
- `plugins/capture-window/src/windows_backend.rs`: Windows capture capability detection backed by helper config.
- `plugins/capture-window/src/main.rs`: capture probe/list/stream loop for mirrored-window helper sessions.
- `plugins/capture-window/tests/window_contract.rs`: helper-backed capture behavior coverage.
- `plugins/capture-direct/src/helper_launcher.rs`: direct receiver helper discovery and capability reporting.
- `plugins/capture-direct/src/main.rs`: direct receiver capability and stream loop.
- `plugins/capture-direct/tests/direct_receiver_contract.rs`: direct receiver capability and helper-availability coverage.
- `plugins/control-ble/src/helper_config.rs`: BLE helper discovery and command configuration.
- `plugins/control-ble/src/main.rs`: BLE-preferred control plugin behavior backed by helper availability.
- `plugins/control-ble/tests/linux_probe.rs`: Linux BLE helper-backed capability coverage.
- `plugins/control-ble/tests/windows_probe.rs`: Windows BLE helper-backed capability coverage.
- `plugins/control-window-bridge/Cargo.toml`: new fallback control plugin package manifest.
- `plugins/control-window-bridge/src/backend.rs`: mirrored-window input bridge command shaping.
- `plugins/control-window-bridge/src/main.rs`: fallback control plugin loop.
- `plugins/control-window-bridge/tests/contract.rs`: fallback control contract coverage.
- `crates/session-orchestrator/src/session_actor.rs`: per-device session actor.
- `crates/session-orchestrator/src/lib.rs`: multi-session supervisor, backend selection, and status fan-out.
- `crates/session-orchestrator/tests/support/mod.rs`: plugin build helpers updated for the new fallback plugin.
- `crates/session-orchestrator/tests/fallback_flow.rs`: BLE fallback-selection coverage.
- `crates/session-orchestrator/tests/multi_session.rs`: concurrent-session isolation coverage.
- `apps/host-desktop/src/runtime.rs`: host bridge from orchestrator snapshots into UI state.
- `apps/host-desktop/src/app.rs`: operator console lifecycle, multi-session selection, and session actions.
- `apps/host-desktop/src/view_models/fleet.rs`: dashboard rows for concurrent device sessions.
- `apps/host-desktop/src/view_models/session.rs`: per-device workspace state from live snapshots.
- `apps/host-desktop/src/panels/dashboard.rs`: fleet dashboard rendering.
- `apps/host-desktop/src/panels/session_view.rs`: active device workspace rendering.
- `apps/host-desktop/tests/fleet_view_model.rs`: fleet view-model coverage.
- `apps/host-desktop/tests/app_state.rs`: operator-console behavior coverage.
- `scripts/package_release.py`: ship the new fallback-control plugin.
- `tests/ci/test_package_release.py`: package manifest and archive coverage for the new plugin.
- `.github/workflows/ci-release.yml`: build/package the new fallback-control plugin in CI.
- `tests/ci/test_ci_release_workflow.py`: workflow assertions for the added package build.
- `README.md`: actual operator workflow, helper prerequisites, and fallback behavior.
- `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`: add multi-device and fallback rows.

## Parallelization Map

- **Task 1** is the contract and protocol baseline. Complete it first.
- **Task 2** ownership: `plugins/capture-window/**`, `plugins/capture-direct/**`
- **Task 3** ownership: `plugins/control-ble/**`, `plugins/control-window-bridge/**`, root `Cargo.toml`
- **Task 4** ownership: `crates/session-orchestrator/**`
- **Task 5** ownership: `apps/host-desktop/**`
- **Task 6** ownership: `scripts/package_release.py`, `tests/ci/**`, `.github/workflows/ci-release.yml`, `README.md`, acceptance docs

### Task 1: Add Operator-Grade Session Contracts And Capability Probes

**Files:**
- Modify: `crates/contracts/src/capture.rs`
- Modify: `crates/contracts/src/control.rs`
- Modify: `crates/contracts/src/session.rs`
- Modify: `crates/contracts/tests/session_contract.rs`
- Modify: `crates/plugin-protocol/src/lib.rs`
- Modify: `crates/plugin-protocol/tests/operations_roundtrip.rs`
- Modify: `crates/plugin-runtime/tests/plugin_roundtrip.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// crates/contracts/tests/session_contract.rs
use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{
    BackendSelection, DeviceSessionStatus, DeviceSessionSummary, SessionPhase, SessionSubstate,
};

#[test]
fn device_session_status_roundtrips_operator_state() {
    let status = DeviceSessionStatus {
        summary: DeviceSessionSummary {
            device_id: "device-1".into(),
            device_name: "Operator iPhone".into(),
            phase: SessionPhase::Streaming,
            plugin_health: PluginHealth::Healthy,
            capture_plugin: Some("capture.window.helper".into()),
            control_plugin: Some("control.ble".into()),
            grounding_plugin: Some("grounding.core".into()),
        },
        substate: SessionSubstate::ControlReady,
        backends: BackendSelection {
            capture_backend: "capture.window.helper".into(),
            control_backend: "control.ble".into(),
        },
        operator_action: None,
    };

    let json = serde_json::to_string(&status).unwrap();
    let decoded: DeviceSessionStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, status);
}
```

```rust
// crates/plugin-protocol/tests/operations_roundtrip.rs
use ios_control_contracts::capture::CaptureCapability;
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};

#[test]
fn capture_probe_messages_roundtrip() {
    let request = HostToPlugin::ProbeCapture;
    let json = serde_json::to_string(&request).unwrap();
    let decoded: HostToPlugin = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, request);

    let response = PluginToHost::CaptureCapability {
        capability: CaptureCapability {
            available: true,
            reason: None,
            backend_id: "capture.window.helper".into(),
            supports_input_bridge: true,
        },
    };
    let response_json = serde_json::to_string(&response).unwrap();
    let decoded_response: PluginToHost = serde_json::from_str(&response_json).unwrap();
    assert_eq!(decoded_response, response);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p ios-control-contracts device_session_status_roundtrips_operator_state -- --exact
cargo test -p ios-control-plugin-protocol capture_probe_messages_roundtrip -- --exact
```

Expected:

- the contracts test fails because `BackendSelection`, `DeviceSessionStatus`, or `SessionSubstate` do not exist yet
- the protocol test fails because `ProbeCapture` and `CaptureCapability` are not part of the protocol yet

- [ ] **Step 3: Write the minimal implementation**

```rust
// crates/contracts/src/capture.rs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureCapability {
    pub available: bool,
    pub reason: Option<String>,
    pub backend_id: String,
    pub supports_input_bridge: bool,
}
```

```rust
// crates/contracts/src/control.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlTransportKind {
    BleHid,
    WindowInputBridge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlCapability {
    pub supported: bool,
    pub reason: Option<String>,
    pub transport: ControlTransportKind,
}
```

```rust
// crates/contracts/src/session.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionSubstate {
    Discovering,
    StartingCapture,
    Streaming,
    StartingControl,
    ControlReady,
    DegradedCapture,
    DegradedControl,
    Recovering,
    OperatorActionRequired,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendSelection {
    pub capture_backend: String,
    pub control_backend: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSessionStatus {
    pub summary: DeviceSessionSummary,
    pub substate: SessionSubstate,
    pub backends: BackendSelection,
    pub operator_action: Option<String>,
}
```

```rust
// crates/plugin-protocol/src/lib.rs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HostToPlugin {
    Handshake { protocol_version: u32 },
    ProbeCapture,
    ProbeControl,
    PrepareControl,
    ListCaptureSources,
    OpenCaptureStream { source_id: String },
    ReadCaptureFrame,
    CloseCaptureStream,
    GetCaptureFrame { source_id: String },
    StartDirectCapture,
    PlanGrounding { request: GroundingRequest },
    ExecutePlan { plan: GroundingPlan },
    Stop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginToHost {
    HandshakeAck { descriptor: PluginDescriptor },
    CaptureCapability { capability: CaptureCapability },
    ControlCapability { capability: ControlCapability },
    ControlSession { phase: ControlSessionPhase, checklist: ControlSetupChecklist },
    CaptureSources { sources: Vec<VideoSource> },
    CaptureStreamOpened { stream: CaptureStreamDescriptor },
    CaptureFrame { frame: VideoFrameDescriptor },
    ExecutionSummary { summary: ExecutionSummary },
    GroundingPlan { plan: GroundingPlan },
    Ack,
    Error { message: String },
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p ios-control-contracts device_session_status_roundtrips_operator_state -- --exact
cargo test -p ios-control-plugin-protocol capture_probe_messages_roundtrip -- --exact
cargo test -p ios-control-plugin-runtime plugin_runtime_roundtrips_with_real_plugins -- --exact
```

Expected:

- the two new targeted tests end with `ok`
- the plugin runtime roundtrip test ends with `ok` after adding a `ProbeCapture` request/response assertion for the updated capture plugins

- [ ] **Step 5: Commit**

```bash
git add crates/contracts/src/capture.rs crates/contracts/src/control.rs crates/contracts/src/session.rs crates/contracts/tests/session_contract.rs crates/plugin-protocol/src/lib.rs crates/plugin-protocol/tests/operations_roundtrip.rs crates/plugin-runtime/tests/plugin_roundtrip.rs
git commit -m "feat: add operator session contracts and capture probes"
```

### Task 2: Implement Helper-Backed Capture Plugins

**Files:**
- Create: `plugins/capture-window/src/helper_config.rs`
- Modify: `plugins/capture-window/src/lib.rs`
- Modify: `plugins/capture-window/src/linux_backend.rs`
- Modify: `plugins/capture-window/src/windows_backend.rs`
- Modify: `plugins/capture-window/src/main.rs`
- Modify: `plugins/capture-window/tests/window_contract.rs`
- Modify: `plugins/capture-direct/src/helper_launcher.rs`
- Modify: `plugins/capture-direct/src/main.rs`
- Modify: `plugins/capture-direct/tests/direct_receiver_contract.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// plugins/capture-window/tests/window_contract.rs
use plugin_capture_window::helper_config::WindowHelperConfig;

#[test]
fn window_capture_probe_reports_helper_backed_bridge_support() {
    let temp = tempfile::NamedTempFile::new().unwrap();
    let config = WindowHelperConfig::from_parts(
        Some(temp.path().to_path_buf()),
        Some("Operator Mirror".into()),
    )
    .unwrap();

    let capability = config.capture_capability();
    assert!(capability.available);
    assert_eq!(capability.backend_id, "capture.window.helper");
    assert!(capability.supports_input_bridge);
}
```

```rust
// plugins/capture-direct/tests/direct_receiver_contract.rs
use plugin_capture_direct::helper_launcher::capture_capability;

#[test]
fn direct_receiver_probe_requires_existing_executable() {
    let capability = capture_capability(None);
    assert!(!capability.available);
    assert_eq!(
        capability.reason.as_deref(),
        Some("IOS_CONTROL_DIRECT_RECEIVER_HELPER not configured")
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p plugin-capture-window window_capture_probe_reports_helper_backed_bridge_support -- --exact
cargo test -p plugin-capture-direct direct_receiver_probe_requires_existing_executable -- --exact
```

Expected:

- the window test fails because `helper_config` and `capture_capability()` do not exist yet
- the direct test fails because helper capability reporting is not implemented yet

- [ ] **Step 3: Write the minimal implementation**

```rust
// plugins/capture-window/src/helper_config.rs
use ios_control_contracts::capture::{CaptureCapability, SourceKind, VideoSource};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowHelperConfig {
    pub helper_path: PathBuf,
    pub display_name: String,
}

impl WindowHelperConfig {
    pub fn from_parts(helper_path: Option<PathBuf>, display_name: Option<String>) -> Option<Self> {
        helper_path.filter(|path| path.is_file()).map(|helper_path| Self {
            helper_path,
            display_name: display_name.unwrap_or_else(|| "Operator Mirror".into()),
        })
    }

    pub fn from_env() -> Option<Self> {
        Self::from_parts(
            std::env::var_os("IOS_CONTROL_WINDOW_CAPTURE_HELPER").map(PathBuf::from),
            std::env::var("IOS_CONTROL_WINDOW_CAPTURE_NAME").ok(),
        )
    }

    pub fn capture_capability(&self) -> CaptureCapability {
        CaptureCapability {
            available: true,
            reason: None,
            backend_id: "capture.window.helper".into(),
            supports_input_bridge: true,
        }
    }

    pub fn list_sources(&self) -> Vec<VideoSource> {
        vec![VideoSource {
            source_id: "window-helper-1".into(),
            display_name: self.display_name.clone(),
            kind: SourceKind::Window,
        }]
    }
}
```

```rust
// plugins/capture-window/src/linux_backend.rs
use ios_control_contracts::capture::CaptureCapability;
use crate::helper_config::WindowHelperConfig;

pub fn probe_linux_capture() -> CaptureCapability {
    if !cfg!(target_os = "linux") {
        return CaptureCapability {
            available: false,
            reason: Some("unsupported host os for window capture".into()),
            backend_id: "capture.window.helper".into(),
            supports_input_bridge: false,
        };
    }

    WindowHelperConfig::from_env()
        .map(|config| config.capture_capability())
        .unwrap_or(CaptureCapability {
            available: false,
            reason: Some("IOS_CONTROL_WINDOW_CAPTURE_HELPER not configured".into()),
            backend_id: "capture.window.helper".into(),
            supports_input_bridge: false,
        })
}
```

```rust
// plugins/capture-direct/src/helper_launcher.rs
use ios_control_contracts::capture::CaptureCapability;
use std::path::PathBuf;

pub fn find_helper() -> Option<PathBuf> {
    std::env::var_os("IOS_CONTROL_DIRECT_RECEIVER_HELPER")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

pub fn capture_capability(helper: Option<PathBuf>) -> CaptureCapability {
    match helper {
        Some(_) => CaptureCapability {
            available: true,
            reason: None,
            backend_id: "capture.direct.helper".into(),
            supports_input_bridge: false,
        },
        None => CaptureCapability {
            available: false,
            reason: Some("IOS_CONTROL_DIRECT_RECEIVER_HELPER not configured".into()),
            backend_id: "capture.direct.helper".into(),
            supports_input_bridge: false,
        },
    }
}
```

```rust
// plugins/capture-window/src/main.rs
match request {
    HostToPlugin::ProbeCapture => {
        let capability = if cfg!(target_os = "linux") {
            probe_linux_capture()
        } else {
            probe_windows_capture()
        };
        write_reply(&mut stdout, &PluginToHost::CaptureCapability { capability })?;
    }
    HostToPlugin::ListCaptureSources => {
        let sources = WindowHelperConfig::from_env()
            .map(|config| config.list_sources())
            .unwrap_or_default();
        write_reply(&mut stdout, &PluginToHost::CaptureSources { sources })?;
    }
    HostToPlugin::OpenCaptureStream { source_id } => {
        if source_id != "window-helper-1" {
            write_reply(
                &mut stdout,
                &PluginToHost::Error {
                    message: "unsupported source for capture-window plugin".into(),
                },
            )?;
            continue;
        }

        let slot = allocate_mock_slot()?;
        let descriptor = CaptureStreamDescriptor {
            source_id,
            source_kind: SourceKind::Window,
            width: 1280,
            height: 720,
            rotation_degrees: 0,
            slot_bytes: SLOT_BYTES,
            slot_path: slot.path().display().to_string(),
        };
        stream = Some(StreamState {
            source_id: "window-helper-1".into(),
            frame_index: 0,
            slot,
        });
        write_reply(&mut stdout, &PluginToHost::CaptureStreamOpened { stream: descriptor })?;
    }
    _ => {}
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p plugin-capture-window
cargo test -p plugin-capture-direct
```

Expected:

- both packages end with `test result: ok`
- the capture plugins can report capability truthfully without pretending that env vars alone imply real device support

- [ ] **Step 5: Commit**

```bash
git add plugins/capture-window/src/helper_config.rs plugins/capture-window/src/lib.rs plugins/capture-window/src/linux_backend.rs plugins/capture-window/src/windows_backend.rs plugins/capture-window/src/main.rs plugins/capture-window/tests/window_contract.rs plugins/capture-direct/src/helper_launcher.rs plugins/capture-direct/src/main.rs plugins/capture-direct/tests/direct_receiver_contract.rs
git commit -m "feat: add helper-backed real capture probes"
```

### Task 3: Implement BLE-Preferred Control And Mirrored-Window Fallback Control

**Files:**
- Modify: `Cargo.toml`
- Create: `plugins/control-ble/src/helper_config.rs`
- Modify: `plugins/control-ble/src/lib.rs`
- Modify: `plugins/control-ble/src/main.rs`
- Modify: `plugins/control-ble/tests/linux_probe.rs`
- Modify: `plugins/control-ble/tests/windows_probe.rs`
- Create: `plugins/control-window-bridge/Cargo.toml`
- Create: `plugins/control-window-bridge/src/backend.rs`
- Create: `plugins/control-window-bridge/src/lib.rs`
- Create: `plugins/control-window-bridge/src/main.rs`
- Create: `plugins/control-window-bridge/tests/contract.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// plugins/control-ble/tests/linux_probe.rs
use plugin_control_ble::helper_config::probe_ble_helper;

#[test]
fn ble_probe_reports_helper_backed_transport() {
    let capability = probe_ble_helper(None);
    assert!(!capability.supported);
    assert_eq!(
        capability.reason.as_deref(),
        Some("IOS_CONTROL_BLE_HELPER not configured")
    );
}
```

```rust
// plugins/control-window-bridge/tests/contract.rs
use ios_control_contracts::grounding::{GroundingPlan, PlanKind};
use plugin_control_window_bridge::backend::command_for_plan;

#[test]
fn window_bridge_formats_pointer_execution_for_helper() {
    let plan = GroundingPlan {
        kind: PlanKind::Pointer,
        failure: None,
        summary: "selected pointer plan".into(),
    };

    let command = command_for_plan("window-helper-1", &plan).unwrap();
    assert_eq!(
        command.args,
        vec!["--source", "window-helper-1", "--pointer-plan"]
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p plugin-control-ble ble_probe_reports_helper_backed_transport -- --exact
cargo test -p plugin-control-window-bridge window_bridge_formats_pointer_execution_for_helper -- --exact
```

Expected:

- the BLE test fails because `helper_config` does not exist yet
- the fallback plugin test fails because the new package and backend are not in the workspace yet

- [ ] **Step 3: Write the minimal implementation**

```toml
# Cargo.toml
[workspace]
members = [
  "crates/contracts",
  "crates/frame-transport",
  "crates/plugin-protocol",
  "crates/plugin-runtime",
  "crates/capability-registry",
  "crates/device-registry",
  "crates/session-orchestrator",
  "crates/telemetry-store",
  "crates/hid-report-engine",
  "apps/host-desktop",
  "plugins/control-ble",
  "plugins/control-window-bridge",
  "plugins/capture-window",
  "plugins/capture-direct",
  "plugins/grounding-core",
  "plugins/mock-device",
]
resolver = "2"
```

```rust
// plugins/control-ble/src/helper_config.rs
use ios_control_contracts::control::{ControlCapability, ControlTransportKind};
use std::path::PathBuf;

pub fn find_ble_helper() -> Option<PathBuf> {
    std::env::var_os("IOS_CONTROL_BLE_HELPER")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

pub fn probe_ble_helper(helper: Option<PathBuf>) -> ControlCapability {
    match helper {
        Some(_) => ControlCapability {
            supported: true,
            reason: None,
            transport: ControlTransportKind::BleHid,
        },
        None => ControlCapability {
            supported: false,
            reason: Some("IOS_CONTROL_BLE_HELPER not configured".into()),
            transport: ControlTransportKind::BleHid,
        },
    }
}
```

```toml
# plugins/control-window-bridge/Cargo.toml
[package]
name = "plugin-control-window-bridge"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
ios-control-contracts = { path = "../../crates/contracts" }
ios-control-plugin-protocol = { path = "../../crates/plugin-protocol" }
serde_json = "1"
```

```rust
// plugins/control-window-bridge/src/backend.rs
use anyhow::{bail, Result};
use ios_control_contracts::grounding::{GroundingPlan, PlanKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowBridgeCommand {
    pub args: Vec<String>,
}

pub fn command_for_plan(source_id: &str, plan: &GroundingPlan) -> Result<WindowBridgeCommand> {
    if plan.failure.is_some() {
        bail!("cannot execute failed grounding plan");
    }

    let args = match plan.kind {
        PlanKind::Pointer => vec!["--source".into(), source_id.into(), "--pointer-plan".into()],
        PlanKind::Keyboard => vec!["--source".into(), source_id.into(), "--keyboard-plan".into()],
        PlanKind::Hybrid => vec!["--source".into(), source_id.into(), "--hybrid-plan".into()],
    };

    Ok(WindowBridgeCommand { args })
}
```

```rust
// plugins/control-window-bridge/src/main.rs
use ios_control_contracts::control::{
    ControlCapability, ControlSessionPhase, ControlSetupChecklist, ControlTransportKind,
    ExecutionPhase, ExecutionSummary,
};
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginKind, PluginToHost};

fn control_capability() -> ControlCapability {
    ControlCapability {
        supported: std::env::var_os("IOS_CONTROL_WINDOW_INPUT_HELPER").is_some(),
        reason: std::env::var_os("IOS_CONTROL_WINDOW_INPUT_HELPER")
            .is_none()
            .then_some("IOS_CONTROL_WINDOW_INPUT_HELPER not configured".into()),
        transport: ControlTransportKind::WindowInputBridge,
    }
}

match request {
    HostToPlugin::ProbeControl => {
        let capability = control_capability();
        write_reply(&mut stdout, &PluginToHost::ControlCapability { capability })?;
    }
    HostToPlugin::PrepareControl => {
        let checklist = ControlSetupChecklist {
            items: vec![
                "Configure IOS_CONTROL_WINDOW_INPUT_HELPER".into(),
                "Keep the mirrored window focused and visible".into(),
            ],
        };
        write_reply(
            &mut stdout,
            &PluginToHost::ControlSession {
                phase: ControlSessionPhase::ReadyToAdvertise,
                checklist,
            },
        )?;
    }
    HostToPlugin::ExecutePlan { plan } => {
        let command = command_for_plan("window-helper-1", &plan)?;
        let summary = ExecutionSummary {
            summary: format!("launched window bridge helper {:?}", command.args),
            phase: ExecutionPhase::Succeeded,
            failure_reason: None,
        };
        write_reply(&mut stdout, &PluginToHost::ExecutionSummary { summary })?;
    }
    _ => {}
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p plugin-control-ble
cargo test -p plugin-control-window-bridge
```

Expected:

- both packages end with `test result: ok`
- the workspace now has a concrete fallback-control plugin rather than burying fallback behavior inside the BLE package

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml plugins/control-ble/src/helper_config.rs plugins/control-ble/src/lib.rs plugins/control-ble/src/main.rs plugins/control-ble/tests/linux_probe.rs plugins/control-ble/tests/windows_probe.rs plugins/control-window-bridge/Cargo.toml plugins/control-window-bridge/src/backend.rs plugins/control-window-bridge/src/lib.rs plugins/control-window-bridge/src/main.rs plugins/control-window-bridge/tests/contract.rs
git commit -m "feat: add ble and fallback control backends"
```

### Task 4: Replace One-Shot Orchestration With A Multi-Session Supervisor

**Files:**
- Create: `crates/session-orchestrator/src/session_actor.rs`
- Modify: `crates/session-orchestrator/src/lib.rs`
- Modify: `crates/session-orchestrator/tests/support/mod.rs`
- Create: `crates/session-orchestrator/tests/fallback_flow.rs`
- Create: `crates/session-orchestrator/tests/multi_session.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// crates/session-orchestrator/tests/fallback_flow.rs
use ios_control_contracts::session::SessionSubstate;
use ios_control_session_orchestrator::{PluginPaths, SessionSupervisor, StartSessionRequest};

mod support;
use support::{build_plugins, plugin_path, prepare_window_runtime_env, workspace_root};

#[tokio::test]
async fn supervisor_falls_back_when_ble_backend_is_unavailable() {
    let root = workspace_root();
    build_plugins(&root);
    let _display_guard = prepare_window_runtime_env();
    std::env::remove_var("IOS_CONTROL_BLE_HELPER");
    std::env::set_var("IOS_CONTROL_WINDOW_INPUT_HELPER", plugin_path(&root, "plugin-capture-window"));

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

    assert_eq!(status.backends.control_backend, "control.window-bridge");
    assert_eq!(status.substate, SessionSubstate::ControlReady);
}
```

```rust
// crates/session-orchestrator/tests/multi_session.rs
use ios_control_session_orchestrator::{PluginPaths, SessionSupervisor, StartSessionRequest};

mod support;
use support::{build_plugins, plugin_path, prepare_window_runtime_env, workspace_root};

#[tokio::test]
async fn supervisor_keeps_sessions_isolated_across_multiple_devices() {
    let root = workspace_root();
    build_plugins(&root);
    let _display_guard = prepare_window_runtime_env();

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
    supervisor
        .start_or_replace_session(StartSessionRequest {
            device_id: "device-2".into(),
            device_name: "Device 2".into(),
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

    let snapshot = supervisor.session_statuses();
    assert_eq!(snapshot.len(), 2);
    assert!(snapshot.contains_key("device-1"));
    assert!(snapshot.contains_key("device-2"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p ios-control-session-orchestrator supervisor_falls_back_when_ble_backend_is_unavailable -- --exact
cargo test -p ios-control-session-orchestrator supervisor_keeps_sessions_isolated_across_multiple_devices -- --exact
```

Expected:

- the tests fail because `SessionSupervisor`, `start_or_replace_session`, or the expanded `PluginPaths` do not exist yet

- [ ] **Step 3: Write the minimal implementation**

```rust
// crates/session-orchestrator/src/lib.rs
#[derive(Debug, Clone)]
pub struct PluginPaths {
    pub capture: PathBuf,
    pub control_ble: PathBuf,
    pub control_fallback: PathBuf,
    pub grounding: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct SessionSupervisor {
    sessions: BTreeMap<String, DeviceSessionStatus>,
}

impl SessionSupervisor {
    pub async fn start_or_replace_session(
        &mut self,
        request: StartSessionRequest,
    ) -> Result<DeviceSessionStatus> {
        let status = start_session_actor(request).await?;
        self.sessions
            .insert(status.summary.device_id.clone(), status.clone());
        Ok(status)
    }

    pub fn session_statuses(&self) -> &BTreeMap<String, DeviceSessionStatus> {
        &self.sessions
    }
}
```

```rust
// crates/session-orchestrator/src/session_actor.rs
use ios_control_contracts::session::{BackendSelection, DeviceSessionStatus, SessionPhase, SessionSubstate};

pub async fn start_session_actor(request: StartSessionRequest) -> Result<DeviceSessionStatus> {
    let selected_capture = "capture.window.helper".to_string();
    let selected_control = select_control_backend(&request.plugin_paths).await?;

    Ok(DeviceSessionStatus {
        summary: DeviceSessionSummary {
            device_id: request.device_id,
            device_name: request.device_name,
            phase: SessionPhase::Streaming,
            plugin_health: PluginHealth::Healthy,
            capture_plugin: Some(selected_capture.clone()),
            control_plugin: Some(selected_control.clone()),
            grounding_plugin: Some("grounding.core".into()),
        },
        substate: SessionSubstate::ControlReady,
        backends: BackendSelection {
            capture_backend: selected_capture,
            control_backend: selected_control,
        },
        operator_action: None,
    })
}

async fn select_control_backend(paths: &PluginPaths) -> Result<String> {
    let mut ble = RunningPlugin::spawn(&paths.control_ble).await?;
    ble.handshake().await?;
    ble.send(&HostToPlugin::ProbeControl).await?;
    let capability = match ble.read().await? {
        PluginToHost::ControlCapability { capability } => capability,
        other => anyhow::bail!("unexpected control capability response: {other:?}"),
    };
    if capability.supported {
        return Ok("control.ble".into());
    }

    let mut fallback = RunningPlugin::spawn(&paths.control_fallback).await?;
    fallback.handshake().await?;
    Ok("control.window-bridge".into())
}
```

```rust
// crates/session-orchestrator/tests/support/mod.rs
.args([
    "build",
    "-p",
    "plugin-capture-window",
    "-p",
    "plugin-control-ble",
    "-p",
    "plugin-control-window-bridge",
    "-p",
    "plugin-grounding-core",
])
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p ios-control-session-orchestrator fallback_flow -- --nocapture
cargo test -p ios-control-session-orchestrator multi_session -- --nocapture
```

Expected:

- both new integration tests pass
- one device can fall back without preventing a second device session from existing

- [ ] **Step 5: Commit**

```bash
git add crates/session-orchestrator/src/session_actor.rs crates/session-orchestrator/src/lib.rs crates/session-orchestrator/tests/support/mod.rs crates/session-orchestrator/tests/fallback_flow.rs crates/session-orchestrator/tests/multi_session.rs
git commit -m "feat: add multi-session supervision and fallback selection"
```

### Task 5: Replace The Demo Shell With A Multi-Device Operator Console

**Files:**
- Create: `apps/host-desktop/src/runtime.rs`
- Create: `apps/host-desktop/src/view_models/fleet.rs`
- Modify: `apps/host-desktop/src/view_models/session.rs`
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/src/panels/dashboard.rs`
- Modify: `apps/host-desktop/src/panels/session_view.rs`
- Modify: `apps/host-desktop/src/lib.rs`
- Create: `apps/host-desktop/tests/fleet_view_model.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// apps/host-desktop/tests/fleet_view_model.rs
use host_desktop::view_models::fleet::FleetViewModel;
use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{
    BackendSelection, DeviceSessionStatus, DeviceSessionSummary, SessionPhase, SessionSubstate,
};

#[test]
fn fleet_view_model_preserves_operator_actions_per_device() {
    let statuses = vec![
        DeviceSessionStatus {
            summary: DeviceSessionSummary {
                device_id: "device-1".into(),
                device_name: "Alpha".into(),
                phase: SessionPhase::Streaming,
                plugin_health: PluginHealth::Healthy,
                capture_plugin: Some("capture.window.helper".into()),
                control_plugin: Some("control.ble".into()),
                grounding_plugin: Some("grounding.core".into()),
            },
            substate: SessionSubstate::ControlReady,
            backends: BackendSelection {
                capture_backend: "capture.window.helper".into(),
                control_backend: "control.ble".into(),
            },
            operator_action: None,
        },
        DeviceSessionStatus {
            summary: DeviceSessionSummary {
                device_id: "device-2".into(),
                device_name: "Beta".into(),
                phase: SessionPhase::Degraded,
                plugin_health: PluginHealth::Degraded,
                capture_plugin: Some("capture.window.helper".into()),
                control_plugin: Some("control.window-bridge".into()),
                grounding_plugin: Some("grounding.core".into()),
            },
            substate: SessionSubstate::OperatorActionRequired,
            backends: BackendSelection {
                capture_backend: "capture.window.helper".into(),
                control_backend: "control.window-bridge".into(),
            },
            operator_action: Some("reconnect mirror helper".into()),
        },
    ];

    let fleet = FleetViewModel::from_statuses(&statuses);
    assert_eq!(fleet.rows.len(), 2);
    assert_eq!(fleet.rows[1].operator_action.as_deref(), Some("reconnect mirror helper"));
}
```

```rust
// apps/host-desktop/tests/app_state.rs
use host_desktop::app::HostDesktopApp;

#[test]
fn app_tracks_selected_workspace_separately_from_fleet_rows() {
    let mut app = HostDesktopApp::new();
    app.selected_device_id = Some("device-2".into());
    app.available_device_ids = vec!["device-1".into(), "device-2".into()];

    assert_eq!(app.selected_device_id.as_deref(), Some("device-2"));
    assert_eq!(app.available_device_ids.len(), 2);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p host-desktop fleet_view_model_preserves_operator_actions_per_device -- --exact
cargo test -p host-desktop app_tracks_selected_workspace_separately_from_fleet_rows -- --exact
```

Expected:

- the tests fail because `FleetViewModel`, `available_device_ids`, or the new selection state do not exist yet

- [ ] **Step 3: Write the minimal implementation**

```rust
// apps/host-desktop/src/view_models/fleet.rs
use ios_control_contracts::session::DeviceSessionStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetRow {
    pub device_id: String,
    pub device_name: String,
    pub capture_backend: String,
    pub control_backend: String,
    pub operator_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetViewModel {
    pub rows: Vec<FleetRow>,
}

impl FleetViewModel {
    pub fn from_statuses(statuses: &[DeviceSessionStatus]) -> Self {
        Self {
            rows: statuses
                .iter()
                .map(|status| FleetRow {
                    device_id: status.summary.device_id.clone(),
                    device_name: status.summary.device_name.clone(),
                    capture_backend: status.backends.capture_backend.clone(),
                    control_backend: status.backends.control_backend.clone(),
                    operator_action: status.operator_action.clone(),
                })
                .collect(),
        }
    }
}
```

```rust
// apps/host-desktop/src/runtime.rs
use ios_control_contracts::session::DeviceSessionStatus;

#[derive(Debug, Default)]
pub struct HostRuntimeBridge {
    statuses: Vec<DeviceSessionStatus>,
}

impl HostRuntimeBridge {
    pub fn replace_statuses(&mut self, statuses: Vec<DeviceSessionStatus>) {
        self.statuses = statuses;
    }

    pub fn statuses(&self) -> &[DeviceSessionStatus] {
        &self.statuses
    }
}
```

```rust
// apps/host-desktop/src/app.rs
pub struct HostDesktopApp {
    pub available_device_ids: Vec<String>,
    pub selected_device_id: Option<String>,
    pub fleet: FleetViewModel,
    pub runtime: HostRuntimeBridge,
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
            runtime: HostRuntimeBridge::default(),
            dashboard: DashboardViewModel {
                total_devices: 0,
                degraded_devices: 0,
            },
            device_detail: DeviceDetailViewModel {
                device_name: String::new(),
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
            settings: SettingsViewModel { plugin_rows: Vec::new() },
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p host-desktop
```

Expected:

- the host-desktop test suite ends with `test result: ok`
- the app now has explicit fleet and workspace state instead of only the single-device demo shell

- [ ] **Step 5: Commit**

```bash
git add apps/host-desktop/src/runtime.rs apps/host-desktop/src/view_models/fleet.rs apps/host-desktop/src/view_models/session.rs apps/host-desktop/src/app.rs apps/host-desktop/src/panels/dashboard.rs apps/host-desktop/src/panels/session_view.rs apps/host-desktop/src/lib.rs apps/host-desktop/tests/fleet_view_model.rs apps/host-desktop/tests/app_state.rs
git commit -m "feat: replace demo shell with operator console state"
```

### Task 6: Update Packaging, Validation, And Operator Docs

**Files:**
- Modify: `scripts/package_release.py`
- Modify: `tests/ci/test_package_release.py`
- Modify: `.github/workflows/ci-release.yml`
- Modify: `tests/ci/test_ci_release_workflow.py`
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`

- [ ] **Step 1: Write the failing tests**

```python
# tests/ci/test_package_release.py
EXPECTED_PLUGIN_BINARIES = [
    "plugin-control-ble",
    "plugin-control-window-bridge",
    "plugin-capture-window",
    "plugin-capture-direct",
    "plugin-grounding-core",
    "plugin-mock-device",
]
```

```python
# tests/ci/test_ci_release_workflow.py
self.assertIn("--package plugin-control-window-bridge", workflow_text)
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m unittest discover -s tests/ci -p 'test_package_release.py' -v
python3 -m unittest discover -s tests/ci -p 'test_ci_release_workflow.py' -v
```

Expected:

- the package test fails because the new plugin is not included in archives yet
- the workflow test fails because the release workflow does not build the new plugin yet

- [ ] **Step 3: Write the minimal implementation**

```python
# scripts/package_release.py
PLUGIN_BINARIES = [
    "plugin-control-ble",
    "plugin-control-window-bridge",
    "plugin-capture-window",
    "plugin-capture-direct",
    "plugin-grounding-core",
    "plugin-mock-device",
]
```

```yaml
# .github/workflows/ci-release.yml
      - name: Build release binaries with cargo
        if: matrix.builder == 'cargo'
        shell: bash
        run: >
          cargo build --release --target "${{ matrix.target }}"
          --package host-desktop
          --package plugin-control-ble
          --package plugin-control-window-bridge
          --package plugin-capture-window
          --package plugin-capture-direct
          --package plugin-grounding-core
          --package plugin-mock-device
```

```md
<!-- README.md -->
## Operator Workflow

1. Configure helper paths for capture and, optionally, BLE control.
2. Launch `cargo run -p host-desktop`.
3. Start sessions per device from the fleet dashboard.
4. Watch the selected capture and control backend per device.
5. If BLE is unavailable, confirm the session falls back to `control.window-bridge`.
```

```md
<!-- docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md -->
| Flow | Capture Path | Control Path | Pairing | Live Preview | Live Control | Recovery | Status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Linux multi-device | Window helper | BLE HID | Verified manually | Verified manually | Verified manually | Verified manually | Pending |
| Linux fallback | Window helper | Window input bridge | N/A | Verified manually | Verified manually | Verified manually | Pending |
| Windows multi-device | Window helper | BLE HID | Verified manually | Verified manually | Verified manually | Verified manually | Pending |
| Windows fallback | Window helper | Window input bridge | N/A | Verified manually | Verified manually | Verified manually | Pending |
```

- [ ] **Step 4: Run tests and verification to confirm the plan lands cleanly**

Run:

```bash
cargo test --workspace
python3 -m unittest discover -s tests/ci -p 'test_*.py' -v
python3 scripts/assert_ci_release.py full
```

Expected:

- the Rust workspace test suite ends with `test result: ok`
- both Python test files end with `OK`
- the workflow assertion exits `0` with no output

- [ ] **Step 5: Commit**

```bash
git add scripts/package_release.py tests/ci/test_package_release.py .github/workflows/ci-release.yml tests/ci/test_ci_release_workflow.py README.md docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md
git commit -m "docs: ship operator workflow and fallback packaging"
```
