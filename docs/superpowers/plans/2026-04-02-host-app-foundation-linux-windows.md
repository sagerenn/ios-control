# Cross-Platform Host App Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Linux/Windows desktop shell, plugin runtime, capability registry, session orchestrator, and diagnostics foundation that the capture, control, and grounding subsystems plug into.

**Architecture:** Use a Rust workspace so the shell, contracts, and plugin runtime share one type system. The desktop app is an `eframe/egui` binary in `apps/host-desktop`, while risky subsystems run as local plugin executables that speak newline-delimited JSON over stdio through a shared protocol crate.

**Tech Stack:** Rust, Cargo workspace, tokio, eframe/egui, serde, thiserror, anyhow, tracing

---

## File Structure

- `Cargo.toml`: workspace manifest for all crates and plugin binaries.
- `rust-toolchain.toml`: pin Rust version for Linux and Windows builds.
- `crates/contracts/src/lib.rs`: re-export contract modules used by shell and plugins.
- `crates/contracts/src/session.rs`: device/session state shared across the app.
- `crates/contracts/src/plugin.rs`: plugin descriptors, health states, and capability summaries.
- `crates/plugin-protocol/src/lib.rs`: stdio IPC request/response types.
- `crates/plugin-runtime/src/lib.rs`: plugin process spawning and handshake logic.
- `crates/capability-registry/src/lib.rs`: host capability cache and probe result model.
- `crates/device-registry/src/lib.rs`: persistent known-device preferences and quirks.
- `crates/session-orchestrator/src/lib.rs`: per-device session graph construction and lifecycle.
- `crates/telemetry-store/src/lib.rs`: persisted diagnostics and session event history.
- `apps/host-desktop/src/lib.rs`: library exports used by UI tests.
- `apps/host-desktop/src/main.rs`: desktop shell entrypoint.
- `apps/host-desktop/src/app.rs`: main egui application state.
- `apps/host-desktop/src/view_models/dashboard.rs`: UI-friendly session summaries.
- `apps/host-desktop/src/panels/dashboard.rs`: multi-session dashboard panel.
- `apps/host-desktop/src/panels/device_detail.rs`: per-device configuration view.
- `apps/host-desktop/src/panels/settings.rs`: advanced configuration and capability details.
- `apps/host-desktop/src/panels/diagnostics.rs`: structured logs and degraded-state view.
- `plugins/mock-device/src/main.rs`: mock plugin used to prove the runtime and orchestrator.

### Task 1: Scaffold The Workspace And Shared Contracts

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `crates/contracts/Cargo.toml`
- Create: `crates/contracts/src/lib.rs`
- Create: `crates/contracts/src/session.rs`
- Create: `crates/contracts/src/plugin.rs`
- Test: `crates/contracts/tests/session_contract.rs`

- [ ] **Step 1: Write the failing test**

```rust
use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{DeviceSessionSummary, SessionPhase};

#[test]
fn device_session_summary_defaults_to_disconnected() {
    let summary = DeviceSessionSummary::new("device-1".into(), "iPhone 15".into());

    assert_eq!(summary.phase, SessionPhase::Disconnected);
    assert_eq!(summary.plugin_health, PluginHealth::Unknown);
    assert!(summary.capture_plugin.is_none());
    assert!(summary.control_plugin.is_none());
    assert!(summary.grounding_plugin.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ios-control-contracts device_session_summary_defaults_to_disconnected -- --exact`
Expected: FAIL with `package ID specification 'ios-control-contracts' did not match any packages`

- [ ] **Step 3: Write minimal implementation**

```toml
# Cargo.toml
[workspace]
members = [
  "crates/contracts",
]
resolver = "2"
```

```toml
# crates/contracts/Cargo.toml
[package]
name = "ios-control-contracts"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
```

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.87.0"
components = ["clippy", "rustfmt"]
```

```rust
// crates/contracts/src/lib.rs
pub mod plugin;
pub mod session;
```

```rust
// crates/contracts/src/plugin.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginHealth {
    Unknown,
    Healthy,
    Degraded,
    Failed,
}
```

```rust
// crates/contracts/src/session.rs
use serde::{Deserialize, Serialize};

use crate::plugin::PluginHealth;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionPhase {
    Disconnected,
    Connecting,
    Streaming,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSessionSummary {
    pub device_id: String,
    pub device_name: String,
    pub phase: SessionPhase,
    pub plugin_health: PluginHealth,
    pub capture_plugin: Option<String>,
    pub control_plugin: Option<String>,
    pub grounding_plugin: Option<String>,
}

impl DeviceSessionSummary {
    pub fn new(device_id: String, device_name: String) -> Self {
        Self {
            device_id,
            device_name,
            phase: SessionPhase::Disconnected,
            plugin_health: PluginHealth::Unknown,
            capture_plugin: None,
            control_plugin: None,
            grounding_plugin: None,
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ios-control-contracts device_session_summary_defaults_to_disconnected -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml rust-toolchain.toml crates/contracts/Cargo.toml crates/contracts/src/lib.rs crates/contracts/src/session.rs crates/contracts/src/plugin.rs crates/contracts/tests/session_contract.rs
git commit -m "chore: scaffold workspace contracts"
```

### Task 2: Add Plugin IPC And A Mock Plugin Runtime Handshake

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/plugin-protocol/Cargo.toml`
- Create: `crates/plugin-protocol/src/lib.rs`
- Create: `crates/plugin-runtime/Cargo.toml`
- Create: `crates/plugin-runtime/src/lib.rs`
- Create: `plugins/mock-device/Cargo.toml`
- Create: `plugins/mock-device/src/main.rs`
- Test: `crates/plugin-runtime/tests/handshake.rs`

- [ ] **Step 1: Write the failing test**

```rust
use std::path::PathBuf;

use ios_control_plugin_runtime::PluginRuntime;

#[tokio::test]
async fn handshake_returns_mock_plugin_descriptor() {
    let runtime = PluginRuntime::new();
    let plugin_path = PathBuf::from("target/debug/plugin-mock-device");

    let descriptor = runtime.handshake(&plugin_path).await.unwrap();

    assert_eq!(descriptor.plugin_id, "mock.device");
    assert_eq!(descriptor.protocol_version, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ios-control-plugin-runtime handshake_returns_mock_plugin_descriptor -- --exact`
Expected: FAIL with `package ID specification 'ios-control-plugin-runtime' did not match any packages`

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/plugin-protocol/src/lib.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginDescriptor {
    pub plugin_id: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HostToPlugin {
    Handshake { protocol_version: u32 },
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginToHost {
    HandshakeAck { descriptor: PluginDescriptor },
}
```

```toml
# Cargo.toml
[workspace]
members = [
  "crates/contracts",
  "crates/plugin-protocol",
  "crates/plugin-runtime",
  "plugins/mock-device",
]
resolver = "2"
```

```toml
# crates/plugin-protocol/Cargo.toml
[package]
name = "ios-control-plugin-protocol"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```toml
# crates/plugin-runtime/Cargo.toml
[package]
name = "ios-control-plugin-runtime"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
ios-control-plugin-protocol = { path = "../plugin-protocol" }
serde_json = "1"
tokio = { version = "1", features = ["macros", "process", "rt-multi-thread", "io-util"] }
```

```rust
// crates/plugin-runtime/src/lib.rs
use std::path::Path;
use std::process::Stdio;

use anyhow::{anyhow, Result};
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginToHost};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

pub struct PluginRuntime;

impl PluginRuntime {
    pub fn new() -> Self {
        Self
    }

    pub async fn handshake(&self, plugin_path: &Path) -> Result<PluginDescriptor> {
        let mut child = Command::new(plugin_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.as_mut().ok_or_else(|| anyhow!("missing stdin"))?;
        let message = serde_json::to_string(&HostToPlugin::Handshake { protocol_version: 1 })?;
        stdin.write_all(message.as_bytes()).await?;
        stdin.write_all(b"\n").await?;

        let stdout = child.stdout.take().ok_or_else(|| anyhow!("missing stdout"))?;
        let mut lines = BufReader::new(stdout).lines();
        let line = lines.next_line().await?.ok_or_else(|| anyhow!("missing reply"))?;

        match serde_json::from_str::<PluginToHost>(&line)? {
            PluginToHost::HandshakeAck { descriptor } => Ok(descriptor),
        }
    }
}
```

```toml
# plugins/mock-device/Cargo.toml
[package]
name = "plugin-mock-device"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
ios-control-plugin-protocol = { path = "../../crates/plugin-protocol" }
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "io-util"] }
```

```rust
// plugins/mock-device/src/main.rs
use ios_control_plugin_protocol::{HostToPlugin, PluginDescriptor, PluginToHost};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut lines = BufReader::new(io::stdin()).lines();
    let mut stdout = io::stdout();

    if let Some(line) = lines.next_line().await? {
        let request: HostToPlugin = serde_json::from_str(&line)?;
        if let HostToPlugin::Handshake { .. } = request {
            let reply = PluginToHost::HandshakeAck {
                descriptor: PluginDescriptor {
                    plugin_id: "mock.device".into(),
                    protocol_version: 1,
                },
            };
            stdout.write_all(serde_json::to_string(&reply)?.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo build -p plugin-mock-device && cargo test -p ios-control-plugin-runtime handshake_returns_mock_plugin_descriptor -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/plugin-protocol/Cargo.toml crates/plugin-protocol/src/lib.rs crates/plugin-runtime/Cargo.toml crates/plugin-runtime/src/lib.rs crates/plugin-runtime/tests/handshake.rs plugins/mock-device/Cargo.toml plugins/mock-device/src/main.rs
git commit -m "feat: add plugin runtime handshake"
```

### Task 3: Add Capability Registry, Device Registry, And Session Orchestrator

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/capability-registry/Cargo.toml`
- Create: `crates/capability-registry/src/lib.rs`
- Create: `crates/device-registry/Cargo.toml`
- Create: `crates/device-registry/src/lib.rs`
- Create: `crates/session-orchestrator/Cargo.toml`
- Create: `crates/session-orchestrator/src/lib.rs`
- Create: `crates/telemetry-store/Cargo.toml`
- Create: `crates/telemetry-store/src/lib.rs`
- Test: `crates/session-orchestrator/tests/session_graph.rs`

- [ ] **Step 1: Write the failing test**

```rust
use ios_control_contracts::session::SessionPhase;
use ios_control_session_orchestrator::{RequestedPlugins, SessionOrchestrator};

#[tokio::test]
async fn build_session_graph_uses_requested_plugins() {
    let orchestrator = SessionOrchestrator::default();
    let summary = orchestrator
        .start_session(
            "device-1",
            RequestedPlugins {
                capture: "capture.mock".into(),
                control: "control.mock".into(),
                grounding: Some("grounding.mock".into()),
            },
        )
        .await
        .unwrap();

    assert_eq!(summary.phase, SessionPhase::Connecting);
    assert_eq!(summary.capture_plugin.as_deref(), Some("capture.mock"));
    assert_eq!(summary.control_plugin.as_deref(), Some("control.mock"));
    assert_eq!(summary.grounding_plugin.as_deref(), Some("grounding.mock"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ios-control-session-orchestrator build_session_graph_uses_requested_plugins -- --exact`
Expected: FAIL with `package ID specification 'ios-control-session-orchestrator' did not match any packages`

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/capability-registry/src/lib.rs
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct CapabilityRegistry {
    entries: BTreeMap<String, bool>,
}

impl CapabilityRegistry {
    pub fn record(&mut self, key: impl Into<String>, value: bool) {
        self.entries.insert(key.into(), value);
    }
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
  "plugins/mock-device",
]
resolver = "2"
```

```toml
# crates/capability-registry/Cargo.toml
[package]
name = "ios-control-capability-registry"
version = "0.1.0"
edition = "2021"
```

```rust
// crates/device-registry/src/lib.rs
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct DeviceRegistry {
    names: BTreeMap<String, String>,
}

impl DeviceRegistry {
    pub fn upsert(&mut self, device_id: impl Into<String>, device_name: impl Into<String>) {
        self.names.insert(device_id.into(), device_name.into());
    }
}
```

```toml
# crates/device-registry/Cargo.toml
[package]
name = "ios-control-device-registry"
version = "0.1.0"
edition = "2021"
```

```rust
// crates/session-orchestrator/src/lib.rs
use anyhow::Result;
use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{DeviceSessionSummary, SessionPhase};

#[derive(Debug, Clone)]
pub struct RequestedPlugins {
    pub capture: String,
    pub control: String,
    pub grounding: Option<String>,
}

#[derive(Debug, Default)]
pub struct SessionOrchestrator;

impl SessionOrchestrator {
    pub async fn start_session(&self, device_id: &str, requested: RequestedPlugins) -> Result<DeviceSessionSummary> {
        Ok(DeviceSessionSummary {
            device_id: device_id.into(),
            device_name: device_id.into(),
            phase: SessionPhase::Connecting,
            plugin_health: PluginHealth::Unknown,
            capture_plugin: Some(requested.capture),
            control_plugin: Some(requested.control),
            grounding_plugin: requested.grounding,
        })
    }
}
```

```toml
# crates/session-orchestrator/Cargo.toml
[package]
name = "ios-control-session-orchestrator"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
ios-control-contracts = { path = "../contracts" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```toml
# crates/telemetry-store/Cargo.toml
[package]
name = "ios-control-telemetry-store"
version = "0.1.0"
edition = "2021"
```

```rust
// crates/telemetry-store/src/lib.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryEvent {
    pub session_id: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct TelemetryStore {
    events: Vec<TelemetryEvent>,
}

impl TelemetryStore {
    pub fn push(&mut self, event: TelemetryEvent) {
        self.events.push(event);
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ios-control-session-orchestrator build_session_graph_uses_requested_plugins -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/capability-registry/Cargo.toml crates/capability-registry/src/lib.rs crates/device-registry/Cargo.toml crates/device-registry/src/lib.rs crates/session-orchestrator/Cargo.toml crates/session-orchestrator/src/lib.rs crates/session-orchestrator/tests/session_graph.rs
git commit -m "feat: add orchestration registries"
```

### Task 4: Add Desktop Shell View Models And Core Panels

**Files:**
- Modify: `Cargo.toml`
- Create: `apps/host-desktop/Cargo.toml`
- Create: `apps/host-desktop/src/lib.rs`
- Create: `apps/host-desktop/src/main.rs`
- Create: `apps/host-desktop/src/app.rs`
- Create: `apps/host-desktop/src/view_models/dashboard.rs`
- Create: `apps/host-desktop/src/panels/dashboard.rs`
- Create: `apps/host-desktop/src/panels/device_detail.rs`
- Create: `apps/host-desktop/src/panels/settings.rs`
- Create: `apps/host-desktop/src/panels/diagnostics.rs`
- Test: `apps/host-desktop/tests/dashboard_view_model.rs`

- [ ] **Step 1: Write the failing test**

```rust
use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{DeviceSessionSummary, SessionPhase};
use host_desktop::view_models::dashboard::DashboardViewModel;

#[test]
fn dashboard_view_model_counts_degraded_sessions() {
    let sessions = vec![
        DeviceSessionSummary {
            device_id: "a".into(),
            device_name: "iPhone".into(),
            phase: SessionPhase::Streaming,
            plugin_health: PluginHealth::Healthy,
            capture_plugin: Some("capture.mock".into()),
            control_plugin: Some("control.mock".into()),
            grounding_plugin: None,
        },
        DeviceSessionSummary {
            device_id: "b".into(),
            device_name: "iPad".into(),
            phase: SessionPhase::Degraded,
            plugin_health: PluginHealth::Degraded,
            capture_plugin: Some("capture.mock".into()),
            control_plugin: Some("control.mock".into()),
            grounding_plugin: None,
        },
    ];

    let view_model = DashboardViewModel::from_sessions(&sessions);

    assert_eq!(view_model.total_devices, 2);
    assert_eq!(view_model.degraded_devices, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p host-desktop dashboard_view_model_counts_degraded_sessions -- --exact`
Expected: FAIL with `package ID specification 'host-desktop' did not match any packages`

- [ ] **Step 3: Write minimal implementation**

```rust
// apps/host-desktop/src/view_models/dashboard.rs
use ios_control_contracts::session::{DeviceSessionSummary, SessionPhase};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardViewModel {
    pub total_devices: usize,
    pub degraded_devices: usize,
}

impl DashboardViewModel {
    pub fn from_sessions(sessions: &[DeviceSessionSummary]) -> Self {
        let degraded_devices = sessions
            .iter()
            .filter(|session| session.phase == SessionPhase::Degraded)
            .count();

        Self {
            total_devices: sessions.len(),
            degraded_devices,
        }
    }
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
  "apps/host-desktop",
  "plugins/mock-device",
]
resolver = "2"
```

```toml
# apps/host-desktop/Cargo.toml
[package]
name = "host-desktop"
version = "0.1.0"
edition = "2021"

[dependencies]
eframe = "0.31"
egui = "0.31"
ios-control-contracts = { path = "../../crates/contracts" }
```

```rust
// apps/host-desktop/src/lib.rs
pub mod app;
pub mod panels {
    pub mod dashboard;
    pub mod device_detail;
    pub mod settings;
    pub mod diagnostics;
}
pub mod view_models {
    pub mod dashboard;
}
```

```rust
// apps/host-desktop/src/panels/dashboard.rs
use egui::Ui;

use crate::view_models::dashboard::DashboardViewModel;

pub fn render(ui: &mut Ui, view_model: &DashboardViewModel) {
    ui.heading("Dashboard");
    ui.label(format!("Devices: {}", view_model.total_devices));
    ui.label(format!("Degraded: {}", view_model.degraded_devices));
}
```

```rust
// apps/host-desktop/src/panels/device_detail.rs
use egui::Ui;

pub fn render(ui: &mut Ui, device_name: &str) {
    ui.heading("Device Detail");
    ui.label(device_name);
}
```

```rust
// apps/host-desktop/src/panels/settings.rs
use egui::Ui;

pub fn render(ui: &mut Ui) {
    ui.heading("Settings");
    ui.label("Advanced configuration goes here");
}
```

```rust
// apps/host-desktop/src/panels/diagnostics.rs
use egui::Ui;

pub fn render(ui: &mut Ui, message: &str) {
    ui.heading("Diagnostics");
    ui.label(message);
}
```

```rust
// apps/host-desktop/src/app.rs
use eframe::egui;

use crate::panels::dashboard;
use crate::view_models::dashboard::DashboardViewModel;

pub struct HostDesktopApp {
    pub dashboard: DashboardViewModel,
}

impl eframe::App for HostDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| dashboard::render(ui, &self.dashboard));
    }
}
```

```rust
// apps/host-desktop/src/main.rs
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "iOS Control Host",
        options,
        Box::new(|_cc| {
            Ok(Box::new(host_desktop::app::HostDesktopApp {
                dashboard: host_desktop::view_models::dashboard::DashboardViewModel {
                    total_devices: 0,
                    degraded_devices: 0,
                },
            }))
        }),
    )
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p host-desktop dashboard_view_model_counts_degraded_sessions -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml apps/host-desktop/Cargo.toml apps/host-desktop/src/lib.rs apps/host-desktop/src/main.rs apps/host-desktop/src/app.rs apps/host-desktop/src/view_models/dashboard.rs apps/host-desktop/src/panels/dashboard.rs apps/host-desktop/src/panels/device_detail.rs apps/host-desktop/src/panels/settings.rs apps/host-desktop/src/panels/diagnostics.rs apps/host-desktop/tests/dashboard_view_model.rs
git commit -m "feat: add host desktop shell"
```
