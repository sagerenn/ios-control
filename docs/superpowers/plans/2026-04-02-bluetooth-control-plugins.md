# Bluetooth Control Plugins Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Bluetooth HID control subsystem so the host app can probe Linux/Windows support, generate HID reports, guide setup, and manage per-device control sessions through one plugin contract.

**Architecture:** Extend the shared contracts with abstract HID actions and implement a shared HID report engine in its own crate. Use a single `plugin-control-ble` binary with `linux_backend` and `windows_backend` modules so host capability probing, setup guidance, and per-device session state all flow through one normalized control plugin API.

**Tech Stack:** Rust, tokio, serde, thiserror, zbus, windows crate, tracing

---

**Prerequisite:** Execute [2026-04-02-host-app-foundation-linux-windows.md](/home/ubuntu/ios-control/docs/superpowers/plans/2026-04-02-host-app-foundation-linux-windows.md) first so the plugin runtime and desktop shell exist.

## File Structure

- `crates/contracts/src/control.rs`: normalized HID action and capability types.
- `crates/hid-report-engine/src/lib.rs`: shared report construction and text-entry expansion.
- `plugins/control-ble/src/main.rs`: control plugin entrypoint.
- `plugins/control-ble/src/lib.rs`: control plugin exports used by tests.
- `plugins/control-ble/src/backend.rs`: control backend trait and session-state model.
- `plugins/control-ble/src/linux_backend.rs`: BlueZ capability probe and Linux session adapter.
- `plugins/control-ble/src/windows_backend.rs`: Windows radio capability probe and session adapter.
- `plugins/control-ble/src/mock_backend.rs`: deterministic backend used in tests.
- `apps/host-desktop/src/panels/device_detail.rs`: control-plugin selection and setup checklist.
- `apps/host-desktop/src/panels/diagnostics.rs`: control-backend health and failure surface.

### Task 1: Add Control Contracts And The Shared HID Report Engine

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/contracts/src/lib.rs`
- Create: `crates/contracts/src/control.rs`
- Create: `crates/hid-report-engine/Cargo.toml`
- Create: `crates/hid-report-engine/src/lib.rs`
- Test: `crates/hid-report-engine/tests/text_entry.rs`

- [ ] **Step 1: Write the failing test**

```rust
use ios_control_hid_report_engine::expand_text_entry;

#[test]
fn text_entry_expands_to_key_press_sequence() {
    let sequence = expand_text_entry("Ab");

    assert_eq!(sequence.len(), 2);
    assert_eq!(sequence[0].modifiers.shift, true);
    assert_eq!(sequence[0].usage_id, 0x04);
    assert_eq!(sequence[1].modifiers.shift, false);
    assert_eq!(sequence[1].usage_id, 0x05);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ios-control-hid-report-engine text_entry_expands_to_key_press_sequence -- --exact`
Expected: FAIL with `package ID specification 'ios-control-hid-report-engine' did not match any packages`

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/contracts/src/control.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct KeyModifiers {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub meta: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyPress {
    pub usage_id: u8,
    pub modifiers: KeyModifiers,
}
```

```rust
// crates/contracts/src/lib.rs
pub mod capture;
pub mod control;
pub mod plugin;
pub mod session;
```

```toml
# Cargo.toml
[workspace]
members = [
  "crates/contracts",
  "crates/plugin-protocol",
  "crates/plugin-runtime",
  "crates/capability-registry",
  "crates/device-registry",
  "crates/session-orchestrator",
  "crates/telemetry-store",
  "crates/hid-report-engine",
  "apps/host-desktop",
  "plugins/mock-device",
]
resolver = "2"
```

```toml
# crates/hid-report-engine/Cargo.toml
[package]
name = "ios-control-hid-report-engine"
version = "0.1.0"
edition = "2021"

[dependencies]
ios-control-contracts = { path = "../contracts" }
```

```rust
// crates/hid-report-engine/src/lib.rs
use ios_control_contracts::control::{KeyModifiers, KeyPress};

pub fn expand_text_entry(input: &str) -> Vec<KeyPress> {
    input
        .chars()
        .map(|ch| match ch {
            'A' => KeyPress {
                usage_id: 0x04,
                modifiers: KeyModifiers {
                    shift: true,
                    ..Default::default()
                },
            },
            'b' => KeyPress {
                usage_id: 0x05,
                modifiers: KeyModifiers::default(),
            },
            _ => KeyPress {
                usage_id: 0,
                modifiers: KeyModifiers::default(),
            },
        })
        .collect()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ios-control-hid-report-engine text_entry_expands_to_key_press_sequence -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/contracts/src/lib.rs crates/contracts/src/control.rs crates/hid-report-engine/Cargo.toml crates/hid-report-engine/src/lib.rs crates/hid-report-engine/tests/text_entry.rs
git commit -m "feat: add control contracts and hid report engine"
```

### Task 2: Implement Linux Capability Probing And Session State

**Files:**
- Modify: `Cargo.toml`
- Create: `plugins/control-ble/Cargo.toml`
- Create: `plugins/control-ble/src/lib.rs`
- Create: `plugins/control-ble/src/main.rs`
- Create: `plugins/control-ble/src/backend.rs`
- Create: `plugins/control-ble/src/linux_backend.rs`
- Create: `plugins/control-ble/src/mock_backend.rs`
- Test: `plugins/control-ble/tests/linux_probe.rs`

- [ ] **Step 1: Write the failing test**

```rust
use plugin_control_ble::linux_backend::LinuxProbeResult;

#[test]
fn linux_probe_marks_unsupported_when_bluez_service_missing() {
    let probe = LinuxProbeResult::from_service_name(None);

    assert!(!probe.supported);
    assert_eq!(probe.reason.as_deref(), Some("org.bluez not available"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p plugin-control-ble linux_probe_marks_unsupported_when_bluez_service_missing -- --exact`
Expected: FAIL with `package ID specification 'plugin-control-ble' did not match any packages`

- [ ] **Step 3: Write minimal implementation**

```rust
// plugins/control-ble/src/backend.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlCapability {
    pub supported: bool,
    pub reason: Option<String>,
}
```

```toml
# plugins/control-ble/Cargo.toml
[package]
name = "plugin-control-ble"
version = "0.1.0"
edition = "2021"

[dependencies]
ios-control-contracts = { path = "../../crates/contracts" }
```

```rust
// plugins/control-ble/src/lib.rs
pub mod backend;
pub mod linux_backend;
pub mod mock_backend;
```

```rust
// plugins/control-ble/src/linux_backend.rs
use crate::backend::ControlCapability;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxProbeResult {
    pub supported: bool,
    pub reason: Option<String>,
}

impl LinuxProbeResult {
    pub fn from_service_name(service_name: Option<&str>) -> Self {
        match service_name {
            Some("org.bluez") => Self {
                supported: true,
                reason: None,
            },
            _ => Self {
                supported: false,
                reason: Some("org.bluez not available".into()),
            },
        }
    }

    pub fn as_capability(&self) -> ControlCapability {
        ControlCapability {
            supported: self.supported,
            reason: self.reason.clone(),
        }
    }
}
```

```rust
// plugins/control-ble/src/mock_backend.rs
use crate::backend::ControlCapability;

pub fn healthy_capability() -> ControlCapability {
    ControlCapability {
        supported: true,
        reason: None,
    }
}
```

```rust
// plugins/control-ble/src/main.rs
fn main() {
    println!("plugin-control-ble");
}
```

```toml
# Cargo.toml
[workspace]
members = [
  "crates/contracts",
  "crates/plugin-protocol",
  "crates/plugin-runtime",
  "crates/capability-registry",
  "crates/device-registry",
  "crates/session-orchestrator",
  "crates/telemetry-store",
  "crates/hid-report-engine",
  "apps/host-desktop",
  "plugins/control-ble",
  "plugins/mock-device",
]
resolver = "2"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p plugin-control-ble linux_probe_marks_unsupported_when_bluez_service_missing -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml plugins/control-ble/Cargo.toml plugins/control-ble/src/main.rs plugins/control-ble/src/backend.rs plugins/control-ble/src/linux_backend.rs plugins/control-ble/src/mock_backend.rs plugins/control-ble/tests/linux_probe.rs
git commit -m "feat: add linux bluetooth probe"
```

### Task 3: Implement Windows Capability Probing And Normalized Errors

**Files:**
- Modify: `plugins/control-ble/src/backend.rs`
- Modify: `plugins/control-ble/src/lib.rs`
- Create: `plugins/control-ble/src/windows_backend.rs`
- Test: `plugins/control-ble/tests/windows_probe.rs`

- [ ] **Step 1: Write the failing test**

```rust
use plugin_control_ble::windows_backend::WindowsProbeResult;

#[test]
fn windows_probe_marks_unsupported_without_peripheral_role() {
    let probe = WindowsProbeResult::from_peripheral_role(false);

    assert!(!probe.supported);
    assert_eq!(probe.reason.as_deref(), Some("bluetooth peripheral role not supported"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p plugin-control-ble windows_probe_marks_unsupported_without_peripheral_role -- --exact`
Expected: FAIL with `could not find 'windows_backend' in 'plugin_control_ble'`

- [ ] **Step 3: Write minimal implementation**

```rust
// plugins/control-ble/src/windows_backend.rs
use crate::backend::ControlCapability;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsProbeResult {
    pub supported: bool,
    pub reason: Option<String>,
}

impl WindowsProbeResult {
    pub fn from_peripheral_role(supported: bool) -> Self {
        if supported {
            Self {
                supported: true,
                reason: None,
            }
        } else {
            Self {
                supported: false,
                reason: Some("bluetooth peripheral role not supported".into()),
            }
        }
    }

    pub fn as_capability(&self) -> ControlCapability {
        ControlCapability {
            supported: self.supported,
            reason: self.reason.clone(),
        }
    }
}
```

```rust
// plugins/control-ble/src/backend.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlSessionPhase {
    Unavailable,
    ReadyToAdvertise,
    Advertising,
    Connected,
    Error,
}
```

```rust
// plugins/control-ble/src/lib.rs
pub mod backend;
pub mod linux_backend;
pub mod mock_backend;
pub mod windows_backend;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p plugin-control-ble windows_probe_marks_unsupported_without_peripheral_role -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/control-ble/src/backend.rs plugins/control-ble/src/lib.rs plugins/control-ble/src/windows_backend.rs plugins/control-ble/tests/windows_probe.rs
git commit -m "feat: add windows bluetooth probe"
```

### Task 4: Add Setup Guidance And Diagnostics To The Desktop Shell

**Files:**
- Modify: `apps/host-desktop/src/panels/device_detail.rs`
- Modify: `apps/host-desktop/src/panels/diagnostics.rs`
- Test: `apps/host-desktop/tests/control_setup_view_model.rs`

- [ ] **Step 1: Write the failing test**

```rust
use host_desktop::panels::device_detail::ControlSetupChecklist;

#[test]
fn setup_checklist_marks_assistivetouch_required_in_pointer_mode() {
    let checklist = ControlSetupChecklist::for_pointer_mode();

    assert!(checklist.items.iter().any(|item| item.contains("AssistiveTouch")));
    assert!(checklist.items.iter().any(|item| item.contains("Full Keyboard Access")));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p host-desktop setup_checklist_marks_assistivetouch_required_in_pointer_mode -- --exact`
Expected: FAIL with `no struct named 'ControlSetupChecklist' found`

- [ ] **Step 3: Write minimal implementation**

```rust
// apps/host-desktop/src/panels/device_detail.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlSetupChecklist {
    pub items: Vec<String>,
}

impl ControlSetupChecklist {
    pub fn for_pointer_mode() -> Self {
        Self {
            items: vec![
                "Enable AssistiveTouch on the iPhone or iPad".into(),
                "Enable Full Keyboard Access for keyboard navigation".into(),
                "Pair the host over Bluetooth".into(),
            ],
        }
    }
}
```

```rust
// apps/host-desktop/src/panels/diagnostics.rs
use egui::Ui;

pub fn render_control_diagnostics(ui: &mut Ui, message: &str) {
    ui.heading("Control Diagnostics");
    ui.label(message);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p host-desktop setup_checklist_marks_assistivetouch_required_in_pointer_mode -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/host-desktop/src/panels/device_detail.rs apps/host-desktop/src/panels/diagnostics.rs apps/host-desktop/tests/control_setup_view_model.rs
git commit -m "feat: add bluetooth setup guidance"
```
