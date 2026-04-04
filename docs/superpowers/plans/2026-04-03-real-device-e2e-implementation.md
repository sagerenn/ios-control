# Real-Device E2E Implementation Plan

> Historical planning artifact. This plan describes a target real-device E2E path from 2026-04-03. It should not be read as evidence that the current branch already provides real-device end-to-end support. For current status, use `README.md`, `docs/TODO.md`, and `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the current mock-backed host/plugin flow into a real-device end-to-end path that can view a real iPhone/iPad screen, drive real BLE input, execute grounded actions, and surface that live state through the desktop host.

**Architecture:** Keep the plugin-oriented Rust workspace and evolve it from one-shot mock orchestration into a long-lived, real-session system. Add live capture/control protocol and frame transport first, then implement real capture and BLE backends, then bridge grounding into execution, and only then wire the host UI around real session lifecycle and real-device validation.

**Tech Stack:** Rust, Cargo workspace, tokio, serde/serde_json, memmap2, eframe/egui, BlueZ/Linux Bluetooth integration, Windows Bluetooth APIs, newline-delimited JSON stdio IPC

---

This plan is intentionally milestone-oriented because the real-device gap spans multiple subsystems. Each task below is a ranked implementation track with concrete files, tests, and checkpoints. Execute in order.

## File Structure

- `crates/contracts/src/capture.rs`: capture contracts for live frame transport metadata.
- `crates/contracts/src/control.rs`: control contracts for executable HID actions and live control state.
- `crates/contracts/src/grounding.rs`: grounding inputs/results used by the execution bridge.
- `crates/contracts/src/session.rs`: session summary/lifecycle types.
- `crates/plugin-protocol/src/lib.rs`: plugin IPC request/response messages.
- `crates/frame-transport/src/lib.rs`: shared memory frame slots or equivalent live frame transport.
- `crates/plugin-runtime/src/lib.rs`: long-lived plugin process lifecycle and handshake/request routing.
- `crates/plugin-runtime/tests/*.rs`: runtime regression coverage for long-lived plugins.
- `plugins/capture-window/src/backend.rs`: real window-capture backend contract.
- `plugins/capture-window/src/linux_backend.rs`: Linux real window capture.
- `plugins/capture-window/src/windows_backend.rs`: Windows real window capture.
- `plugins/capture-window/src/main.rs`: plugin loop exposing live capture over protocol.
- `plugins/capture-direct/src/backend.rs`: real direct-receiver backend contract.
- `plugins/capture-direct/src/helper_launcher.rs`: helper discovery and launch path.
- `plugins/capture-direct/src/main.rs`: plugin loop exposing direct capture over protocol.
- `plugins/control-ble/src/backend.rs`: real control-session state and transport model.
- `plugins/control-ble/src/linux_backend.rs`: Linux BLE peripheral/HID backend.
- `plugins/control-ble/src/windows_backend.rs`: Windows BLE peripheral/HID backend.
- `plugins/control-ble/src/main.rs`: plugin loop exposing real control operations over protocol.
- `plugins/grounding-core/src/action_selector.rs`: choose plans from target state.
- `plugins/grounding-core/src/execution_monitor.rs`: observe execution success/failure.
- `plugins/grounding-core/src/recovery_controller.rs`: bounded retries and failure escalation.
- `plugins/grounding-core/src/main.rs`: plugin loop exposing grounding/execution planning.
- `crates/session-orchestrator/src/lib.rs`: real long-lived session lifecycle, orchestration, and cleanup.
- `crates/session-orchestrator/tests/*.rs`: mock/live-path orchestration tests.
- `apps/host-desktop/src/app.rs`: host app session lifecycle and UI state.
- `apps/host-desktop/src/main.rs`: real host startup path.
- `apps/host-desktop/src/view_models/*.rs`: UI-facing session, capture, control, diagnostics state.
- `apps/host-desktop/src/panels/*.rs`: live preview, control setup, diagnostics, and settings rendering.
- `apps/host-desktop/tests/*.rs`: state and rendering regressions.
- `README.md`: real-device usage instructions and supported developer flows.

### Task 1: Add Live Session Protocol And Frame Transport

**Files:**
- Modify: `crates/contracts/src/capture.rs`
- Modify: `crates/contracts/src/control.rs`
- Modify: `crates/contracts/src/session.rs`
- Modify: `crates/plugin-protocol/src/lib.rs`
- Modify: `crates/frame-transport/src/lib.rs`
- Test: `crates/plugin-protocol/tests/operations_roundtrip.rs`
- Test: `crates/plugin-runtime/tests/plugin_roundtrip.rs`

- [ ] **Step 1: Write the failing test**

```rust
use ios_control_contracts::capture::VideoFrameDescriptor;
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};

#[test]
fn capture_stream_messages_roundtrip() {
    let request = HostToPlugin::OpenCaptureStream {
        source_id: "window-1".into(),
    };
    let json = serde_json::to_string(&request).unwrap();
    let decoded: HostToPlugin = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, request);

    let response = PluginToHost::CaptureFrame {
        frame: VideoFrameDescriptor {
            source_id: "window-1".into(),
            source_kind: ios_control_contracts::capture::SourceKind::Window,
            width: 1280,
            height: 720,
            rotation_degrees: 0,
            frame_index: 7,
            health: ios_control_contracts::capture::FrameHealth::Healthy,
        },
    };
    let response_json = serde_json::to_string(&response).unwrap();
    let decoded_response: PluginToHost = serde_json::from_str(&response_json).unwrap();
    assert_eq!(decoded_response, response);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ios-control-plugin-protocol capture_stream_messages_roundtrip -- --exact`
Expected: FAIL because `OpenCaptureStream` does not exist yet.

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/plugin-protocol/src/lib.rs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HostToPlugin {
    Handshake { protocol_version: u32 },
    ProbeControl,
    PrepareControl,
    ListCaptureSources,
    OpenCaptureStream { source_id: String },
    ReadCaptureFrame,
    CloseCaptureStream,
    GetCaptureFrame { source_id: String },
    StartDirectCapture,
    PlanGrounding { request: GroundingRequest },
    ExecutePlan { summary: String },
    Stop,
}
```

```rust
// crates/frame-transport/src/lib.rs
use anyhow::{bail, Result};
use memmap2::MmapMut;

pub struct FrameSlot {
    mmap: MmapMut,
    byte_len: usize,
}

impl FrameSlot {
    pub fn new(byte_len: usize) -> Result<Self> {
        Ok(Self {
            mmap: MmapMut::map_anon(byte_len)?,
            byte_len,
        })
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > self.byte_len {
            bail!("frame larger than slot");
        }
        self.mmap[..bytes.len()].copy_from_slice(bytes);
        Ok(())
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ios-control-plugin-protocol capture_stream_messages_roundtrip -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/contracts/src/capture.rs crates/contracts/src/control.rs crates/contracts/src/session.rs crates/plugin-protocol/src/lib.rs crates/frame-transport/src/lib.rs crates/plugin-protocol/tests/operations_roundtrip.rs crates/plugin-runtime/tests/plugin_roundtrip.rs
git commit -m "feat: add live session capture and execution protocol"
```

### Task 2: Implement Real Window And Direct Capture Paths

**Files:**
- Modify: `plugins/capture-window/src/backend.rs`
- Modify: `plugins/capture-window/src/linux_backend.rs`
- Modify: `plugins/capture-window/src/windows_backend.rs`
- Modify: `plugins/capture-window/src/main.rs`
- Modify: `plugins/capture-direct/src/backend.rs`
- Modify: `plugins/capture-direct/src/helper_launcher.rs`
- Modify: `plugins/capture-direct/src/main.rs`
- Test: `plugins/capture-window/tests/window_contract.rs`
- Test: `plugins/capture-direct/tests/direct_receiver_contract.rs`

- [ ] **Step 1: Write the failing test**

```rust
use plugin_capture_window::linux_backend::probe_linux_capture;

#[test]
fn linux_capture_probe_requires_runtime_support() {
    let supported = probe_linux_capture();
    assert!(!supported, "default test environment should not claim real capture support");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p plugin-capture-window linux_capture_probe_requires_runtime_support -- --exact`
Expected: FAIL or produce a meaningless `cfg!`-only answer, showing the probe is not a real runtime check.

- [ ] **Step 3: Write minimal implementation**

```rust
// plugins/capture-window/src/linux_backend.rs
pub fn probe_linux_capture() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some()
}
```

```rust
// plugins/capture-direct/src/helper_launcher.rs
use std::path::PathBuf;

pub fn find_helper() -> Option<PathBuf> {
    std::env::var_os("IOS_CONTROL_DIRECT_RECEIVER_HELPER").map(PathBuf::from)
}
```

Add the real follow-up code in this task for:
- opening a live capture stream instead of returning one fake descriptor
- feeding frame bytes into `FrameSlot`
- surfacing `Healthy` / `Stalled` / `Resized` from real runtime conditions

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p plugin-capture-window
cargo test -p plugin-capture-direct
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/capture-window/src/backend.rs plugins/capture-window/src/linux_backend.rs plugins/capture-window/src/windows_backend.rs plugins/capture-window/src/main.rs plugins/capture-direct/src/backend.rs plugins/capture-direct/src/helper_launcher.rs plugins/capture-direct/src/main.rs plugins/capture-window/tests/window_contract.rs plugins/capture-direct/tests/direct_receiver_contract.rs
git commit -m "feat: add real capture backend foundations"
```

### Task 3: Implement Real BLE HID Peripheral Sessions

**Files:**
- Modify: `plugins/control-ble/src/backend.rs`
- Modify: `plugins/control-ble/src/linux_backend.rs`
- Modify: `plugins/control-ble/src/windows_backend.rs`
- Modify: `plugins/control-ble/src/main.rs`
- Modify: `crates/hid-report-engine/src/lib.rs`
- Test: `plugins/control-ble/tests/linux_probe.rs`
- Test: `plugins/control-ble/tests/windows_probe.rs`
- Test: `crates/hid-report-engine/tests/text_entry.rs`

- [ ] **Step 1: Write the failing test**

```rust
use plugin_control_ble::linux_backend::LinuxProbeResult;

#[test]
fn linux_probe_reports_reason_when_bluez_missing() {
    let probe = LinuxProbeResult::from_service_name(None);
    assert_eq!(probe.reason.as_deref(), Some("org.bluez not available"));
}
```

- [ ] **Step 2: Run test to verify it fails or is insufficient**

Run: `cargo test -p plugin-control-ble linux_probe_reports_reason_when_bluez_missing -- --exact`
Expected: existing probe tests pass, but there is still no real advertise/connect/report path.

- [ ] **Step 3: Write minimal implementation**

Implement in this task:
- real `ProbeControl`
- real `PrepareControl`
- a new long-lived control session state machine for advertise/connect/disconnect
- HID report submission path using the shared report engine

```rust
// plugins/control-ble/src/backend.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlSessionState {
    Unsupported,
    Ready,
    Advertising,
    Connected,
    Error(String),
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p plugin-control-ble
cargo test -p ios-control-hid-report-engine
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/control-ble/src/backend.rs plugins/control-ble/src/linux_backend.rs plugins/control-ble/src/windows_backend.rs plugins/control-ble/src/main.rs crates/hid-report-engine/src/lib.rs plugins/control-ble/tests/linux_probe.rs plugins/control-ble/tests/windows_probe.rs crates/hid-report-engine/tests/text_entry.rs
git commit -m "feat: add real ble hid session flow"
```

### Task 4: Bridge Grounding Plans Into Real Execution

**Files:**
- Modify: `plugins/grounding-core/src/action_selector.rs`
- Modify: `plugins/grounding-core/src/execution_monitor.rs`
- Modify: `plugins/grounding-core/src/recovery_controller.rs`
- Modify: `plugins/grounding-core/src/main.rs`
- Modify: `crates/session-orchestrator/src/lib.rs`
- Test: `plugins/grounding-core/tests/plans.rs`
- Test: `crates/session-orchestrator/tests/mock_flow.rs`

- [ ] **Step 1: Write the failing test**

```rust
use ios_control_contracts::grounding::PlanKind;
use plugin_grounding_core::action_selector::ActionSelector;
use plugin_grounding_core::focus_tracker::FocusTracker;

#[test]
fn keyboard_plan_wins_when_keyboard_preferred() {
    let selector = ActionSelector::default();
    let focus = FocusTracker {
        focus_confidence: 0.9,
        keyboard_friendly: true,
    };
    let plan = selector.choose_plan(false, &focus, 999.0).unwrap();
    assert_eq!(plan.kind, PlanKind::Keyboard);
}
```

- [ ] **Step 2: Run test to verify current behavior is too narrow**

Run: `cargo test -p plugin-grounding-core keyboard_plan_wins_when_keyboard_preferred -- --exact`
Expected: PASS or near-PASS, but there is still no execution bridge into the control plugin.

- [ ] **Step 3: Write minimal implementation**

Implement in this task:
- an execution request path from orchestrator to control
- execution-monitor feedback on whether the screen changed
- bounded retry / recovery on mismatch

```rust
// crates/session-orchestrator/src/lib.rs
pub struct ExecutionResult {
    pub applied: bool,
    pub summary: String,
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p plugin-grounding-core
cargo test -p ios-control-session-orchestrator
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/grounding-core/src/action_selector.rs plugins/grounding-core/src/execution_monitor.rs plugins/grounding-core/src/recovery_controller.rs plugins/grounding-core/src/main.rs crates/session-orchestrator/src/lib.rs plugins/grounding-core/tests/plans.rs crates/session-orchestrator/tests/mock_flow.rs
git commit -m "feat: execute grounding plans through live control path"
```

### Task 5: Replace Demo UI With Real Session UI

**Files:**
- Modify: `apps/host-desktop/src/main.rs`
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/src/view_models/device_detail.rs`
- Modify: `apps/host-desktop/src/view_models/session.rs`
- Modify: `apps/host-desktop/src/view_models/diagnostics.rs`
- Modify: `apps/host-desktop/src/view_models/settings.rs`
- Modify: `apps/host-desktop/src/panels/device_detail.rs`
- Modify: `apps/host-desktop/src/panels/session_view.rs`
- Modify: `apps/host-desktop/src/panels/settings.rs`
- Test: `apps/host-desktop/tests/app_state.rs`
- Test: `apps/host-desktop/tests/capture_source_view_model.rs`

- [ ] **Step 1: Write the failing test**

```rust
use host_desktop::app::HostDesktopApp;

#[test]
fn demo_shell_does_not_pretend_a_live_session_exists() {
    let app = HostDesktopApp::demo();
    assert!(app.session.selected_source.is_none());
    assert!(app.session.latest_frame.is_none());
}
```

- [ ] **Step 2: Run test to verify the current demo-only shell is insufficient**

Run: `cargo test -p host-desktop demo_shell_does_not_pretend_a_live_session_exists -- --exact`
Expected: PASS, but the host still does not start a real orchestrated session from UI actions.

- [ ] **Step 3: Write minimal implementation**

Implement in this task:
- a real start-session action path from the host shell
- display of live frame/control/grounding state
- explicit stop/shutdown controls
- host error display for missing permissions/backends

```rust
// apps/host-desktop/src/view_models/session.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionUiState {
    Idle,
    Starting,
    Streaming,
    Error(String),
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p host-desktop`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/host-desktop/src/main.rs apps/host-desktop/src/app.rs apps/host-desktop/src/view_models/device_detail.rs apps/host-desktop/src/view_models/session.rs apps/host-desktop/src/view_models/diagnostics.rs apps/host-desktop/src/view_models/settings.rs apps/host-desktop/src/panels/device_detail.rs apps/host-desktop/src/panels/session_view.rs apps/host-desktop/src/panels/settings.rs apps/host-desktop/tests/app_state.rs apps/host-desktop/tests/capture_source_view_model.rs
git commit -m "feat: replace demo shell with real session ui"
```

### Task 6: Add Real-Device Validation And Operator Docs

**Files:**
- Modify: `README.md`
- Modify: `crates/session-orchestrator/tests/local_mock_e2e.rs`
- Create: `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn operator_docs_reference_real_device_flow() {
    let readme = std::fs::read_to_string("README.md").unwrap();
    assert!(readme.contains("real device"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ios-control-session-orchestrator operator_docs_reference_real_device_flow -- --exact`
Expected: FAIL because there is no real-device operator matrix yet.

- [ ] **Step 3: Write minimal implementation**

Document and test:
- supported host OSes and adapter expectations
- required iOS setup
- successful pairing/capture/control checklist
- reconnect / failure diagnostics
- known unsupported paths

```markdown
# Real Device Acceptance Matrix

- Host OS
- Bluetooth adapter
- Capture path
- Device type
- Pairing result
- Live preview result
- Keyboard result
- Pointer result
- Recovery result
```

- [ ] **Step 4: Run verification**

Run:

```bash
cargo test --workspace
python3 -m unittest discover -s tests/ci -p 'test_*.py' -v
python3 scripts/assert_ci_release.py full
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add README.md crates/session-orchestrator/tests/local_mock_e2e.rs docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md
git commit -m "docs: add real device validation guidance"
```
