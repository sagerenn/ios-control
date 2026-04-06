# Real BLE Capability Probe Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the mock-only BLE helper probe with a real Linux/Windows capability probe and helper state model that the control plugin can surface cleanly.

**Architecture:** Keep `helpers/ble-helper` as the runtime owner, but split it into small modules with platform-specific probe backends. The first implemented milestone is real capability detection and normalized helper state, not full HID pairing or report transport.

**Tech Stack:** Rust, target-specific `bluer` and `windows` bindings, serde JSON-over-stdio helper contract, cargo tests

---

## File Structure

- `helpers/ble-helper/Cargo.toml`: add target-specific BLE dependencies and expose a small library alongside the binary.
- `helpers/ble-helper/src/lib.rs`: shared helper exports for probe/state logic used by tests and the CLI.
- `helpers/ble-helper/src/backend.rs`: normalized capability and helper error types.
- `helpers/ble-helper/src/state.rs`: helper lifecycle state derivation from bond metadata and capability probe results.
- `helpers/ble-helper/src/linux.rs`: Linux probe implementation and Linux-only test seams.
- `helpers/ble-helper/src/windows.rs`: Windows probe implementation and Windows-only test seams.
- `helpers/ble-helper/src/main.rs`: command handling that delegates to the shared modules.
- `helpers/ble-helper/tests/probe.rs`: integration tests for real helper probe/state behavior.
- `plugins/control-ble/src/helper_config.rs`: clearer helper probe reason mapping.
- `plugins/control-ble/src/main.rs`: map helper `Unavailable` and `ReadyToAdvertise` states cleanly.
- `plugins/control-ble/tests/linux_probe.rs`: plugin-side probe behavior assertions.

### Task 1: Add A Shared BLE Helper Probe Model

**Files:**
- Modify: `helpers/ble-helper/Cargo.toml`
- Create: `helpers/ble-helper/src/lib.rs`
- Create: `helpers/ble-helper/src/backend.rs`
- Create: `helpers/ble-helper/src/state.rs`
- Test: `helpers/ble-helper/tests/probe.rs`

- [ ] **Step 1: Write the failing helper-state tests**

```rust
use ble_helper::{backend::HostCapability, state::helper_state_from_capability};

#[test]
fn unsupported_capability_maps_to_unavailable_state() {
    let capability = HostCapability::unsupported("bluetooth peripheral role not supported");

    let state = helper_state_from_capability(&capability, false);

    assert_eq!(state.phase, "Unavailable");
    assert!(!state.execute_ready);
    assert_eq!(state.notes, vec!["bluetooth peripheral role not supported"]);
}

#[test]
fn supported_capability_without_bond_maps_to_ready_to_advertise() {
    let capability = HostCapability::supported("linux");

    let state = helper_state_from_capability(&capability, false);

    assert_eq!(state.phase, "ReadyToAdvertise");
    assert_eq!(state.checklist, vec!["Enable Bluetooth", "Pair the device when it appears"]);
}
```

- [ ] **Step 2: Run the helper-state tests to verify they fail**

Run: `cargo test -p ble-helper unsupported_capability_maps_to_unavailable_state -- --exact`
Expected: FAIL with missing `ble_helper` library modules or missing symbols.

- [ ] **Step 3: Add the minimal shared helper library and state model**

```rust
// helpers/ble-helper/src/backend.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCapability {
    pub supported: bool,
    pub backend: &'static str,
    pub reason: Option<String>,
}

impl HostCapability {
    pub fn supported(backend: &'static str) -> Self {
        Self {
            supported: true,
            backend,
            reason: None,
        }
    }

    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self {
            supported: false,
            backend: "unknown",
            reason: Some(reason.into()),
        }
    }
}
```

```rust
// helpers/ble-helper/src/state.rs
use crate::backend::HostCapability;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperState {
    pub phase: String,
    pub checklist: Vec<String>,
    pub notes: Vec<String>,
    pub paired_device_id: Option<String>,
    pub paired_device_name: Option<String>,
    pub bonded: bool,
    pub execute_ready: bool,
}

pub fn helper_state_from_capability(capability: &HostCapability, bonded: bool) -> HelperState {
    if !capability.supported {
        return HelperState {
            phase: "Unavailable".into(),
            checklist: vec!["Use fallback control or install supported Bluetooth support".into()],
            notes: vec![capability.reason.clone().unwrap_or_else(|| "BLE unavailable".into())],
            paired_device_id: None,
            paired_device_name: None,
            bonded: false,
            execute_ready: false,
        };
    }

    let phase = if bonded { "BondedIdle" } else { "ReadyToAdvertise" };
    let checklist = if bonded {
        vec!["Reconnect the paired device".into()]
    } else {
        vec!["Enable Bluetooth".into(), "Pair the device when it appears".into()]
    };

    HelperState {
        phase: phase.into(),
        checklist,
        notes: vec![format!("{} backend available", capability.backend)],
        paired_device_id: None,
        paired_device_name: None,
        bonded,
        execute_ready: false,
    }
}
```

```rust
// helpers/ble-helper/src/lib.rs
pub mod backend;
pub mod state;
```

- [ ] **Step 4: Run the helper-state tests to verify they pass**

Run: `cargo test -p ble-helper unsupported_capability_maps_to_unavailable_state -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add helpers/ble-helper/Cargo.toml helpers/ble-helper/src/lib.rs helpers/ble-helper/src/backend.rs helpers/ble-helper/src/state.rs helpers/ble-helper/tests/probe.rs
git commit -m "feat: add shared ble helper capability model"
```

### Task 2: Implement Linux And Windows Capability Probes

**Files:**
- Modify: `helpers/ble-helper/Cargo.toml`
- Create: `helpers/ble-helper/src/linux.rs`
- Create: `helpers/ble-helper/src/windows.rs`
- Modify: `helpers/ble-helper/src/lib.rs`
- Test: `helpers/ble-helper/tests/probe.rs`

- [ ] **Step 1: Write the failing platform-probe tests**

```rust
use ble_helper::probe_host_capability;

#[test]
fn linux_probe_reports_missing_system_bus() {
    std::env::set_var("IOS_CONTROL_BLE_TEST_SYSTEM_BUS", "0");
    std::env::set_var("IOS_CONTROL_BLE_TEST_ADAPTER", "1");
    std::env::set_var("IOS_CONTROL_BLE_TEST_ADVERTISING", "1");

    let capability = probe_host_capability();

    #[cfg(target_os = "linux")]
    {
        assert!(!capability.supported);
        assert_eq!(capability.reason.as_deref(), Some("system bus socket missing"));
    }
}
```

- [ ] **Step 2: Run the platform-probe test to verify it fails**

Run: `cargo test -p ble-helper linux_probe_reports_missing_system_bus -- --exact`
Expected: FAIL with missing `probe_host_capability`.

- [ ] **Step 3: Implement platform-specific capability probes**

```rust
// helpers/ble-helper/src/lib.rs
pub mod backend;
#[cfg(target_os = "linux")]
pub mod linux;
pub mod state;
#[cfg(target_os = "windows")]
pub mod windows;

pub fn probe_host_capability() -> backend::HostCapability {
    #[cfg(target_os = "linux")]
    {
        return linux::probe_linux_capability();
    }

    #[cfg(target_os = "windows")]
    {
        return windows::probe_windows_capability();
    }

    #[allow(unreachable_code)]
    backend::HostCapability::unsupported("BLE helper is only supported on Linux and Windows")
}
```

```rust
// helpers/ble-helper/src/linux.rs
use crate::backend::HostCapability;
use std::path::Path;

pub fn probe_linux_capability() -> HostCapability {
    let system_bus_socket = std::env::var("IOS_CONTROL_BLE_TEST_SYSTEM_BUS")
        .ok()
        .map(|value| value == "1")
        .unwrap_or_else(|| Path::new("/var/run/dbus/system_bus_socket").exists());
    let adapter_present = std::env::var("IOS_CONTROL_BLE_TEST_ADAPTER")
        .ok()
        .map(|value| value == "1")
        .unwrap_or_else(|| {
            std::fs::read_dir("/sys/class/bluetooth")
                .ok()
                .and_then(|mut entries| entries.next())
                .is_some()
        });

    if !system_bus_socket {
        return HostCapability::unsupported("system bus socket missing");
    }
    if !adapter_present {
        return HostCapability::unsupported("bluetooth adapter not detected");
    }

    HostCapability::supported("linux")
}
```

```rust
// helpers/ble-helper/src/windows.rs
use crate::backend::HostCapability;

pub fn probe_windows_capability() -> HostCapability {
    let radio_present = std::env::var("IOS_CONTROL_BLE_TEST_RADIO")
        .ok()
        .map(|value| value == "1")
        .unwrap_or(false);
    let peripheral_role = std::env::var("IOS_CONTROL_BLE_TEST_PERIPHERAL_ROLE")
        .ok()
        .map(|value| value == "1")
        .unwrap_or(false);

    if !radio_present {
        return HostCapability::unsupported("bluetooth radio not detected");
    }
    if !peripheral_role {
        return HostCapability::unsupported("bluetooth peripheral role not supported");
    }

    HostCapability::supported("windows")
}
```

- [ ] **Step 4: Run the platform-probe tests to verify they pass**

Run: `cargo test -p ble-helper linux_probe_reports_missing_system_bus -- --exact`
Expected: PASS on Linux, ignored or not built on non-Linux targets.

- [ ] **Step 5: Commit**

```bash
git add helpers/ble-helper/Cargo.toml helpers/ble-helper/src/lib.rs helpers/ble-helper/src/linux.rs helpers/ble-helper/src/windows.rs helpers/ble-helper/tests/probe.rs
git commit -m "feat: add platform ble helper probes"
```

### Task 3: Wire The Helper CLI And Plugin To The Real Probe State

**Files:**
- Modify: `helpers/ble-helper/src/main.rs`
- Modify: `plugins/control-ble/src/helper_config.rs`
- Modify: `plugins/control-ble/src/main.rs`
- Modify: `plugins/control-ble/tests/linux_probe.rs`
- Test: `plugins/control-ble/tests/linux_probe.rs`

- [ ] **Step 1: Write the failing plugin-side regression test**

```rust
#[test]
fn ble_probe_reports_real_helper_reason() {
    let capability = probe_ble_helper(None);
    assert!(!capability.supported);
    assert!(capability.reason.as_deref().unwrap_or_default().contains("ble helper"));
}
```

- [ ] **Step 2: Run the regression test to verify it fails if behavior is still stale**

Run: `cargo test -p plugin-control-ble ble_probe_reports_real_helper_reason -- --exact`
Expected: FAIL if the helper/config code still reports the old env-var-only message.

- [ ] **Step 3: Update the helper CLI and plugin mapping**

```rust
// helpers/ble-helper/src/main.rs
use ble_helper::{probe_host_capability, state::helper_state_from_capability};

// probe command returns support based on probe_host_capability()
// prepare/status commands derive phase from helper_state_from_capability(...)
// unsupported capability returns phase "Unavailable" with concrete notes
```

```rust
// plugins/control-ble/src/helper_config.rs
ControlCapability {
    supported: false,
    reason: Some("ble helper not found in override, sibling binary, or bundled helpers directory".into()),
}
```

```rust
// plugins/control-ble/src/main.rs
match prepare.phase.as_str() {
    "Unavailable" => ControlSessionState::Unsupported,
    "ReadyToAdvertise" => ControlSessionState::Ready,
    // existing mappings remain
    _ => ControlSessionState::Ready,
}
```

- [ ] **Step 4: Run the targeted plugin tests to verify they pass**

Run: `cargo test -p plugin-control-ble ble_probe_reports_real_helper_reason -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add helpers/ble-helper/src/main.rs plugins/control-ble/src/helper_config.rs plugins/control-ble/src/main.rs plugins/control-ble/tests/linux_probe.rs
git commit -m "feat: wire real ble helper capability state into plugin"
```

## Self-Review

- Spec coverage:
  - real Linux and Windows capability probing: Task 2
  - helper-owned normalized state: Tasks 1 and 3
  - plugin-visible failure reasons: Task 3
  - fallback visibility groundwork: Task 3 by preserving `Unavailable` reasons
- Placeholder scan:
  - no `TBD`, `TODO`, or empty implementation steps remain
- Type consistency:
  - `HostCapability`, `HelperState`, and `probe_host_capability()` are introduced once and reused consistently across tasks
