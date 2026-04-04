# End-To-End Product Completion Implementation Plan

> Historical planning artifact. This plan describes a target mock-backed product-completion path from 2026-04-03. It does not mean the current branch is fully wired today. For current status, use `README.md` and `docs/TODO.md`.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the current scaffold into a fully wired, mock-backed end-to-end product flow where the host can discover plugins, start a local session, render capture/control/grounding state, and verify the entire path through tests and docs.

**Architecture:** Keep the existing Rust workspace and plugin boundaries, but finish the missing runtime path between host and plugins. Extend the shared contracts and stdio protocol first, then implement stateful plugin executables, wire the host-side registries and orchestrator, and finally connect the desktop shell to that runtime with a mock-backed end-to-end path that is runnable on Linux and Windows without real iOS hardware.

**Tech Stack:** Rust, Cargo workspace, tokio, serde, serde_json, anyhow, eframe/egui, newline-delimited JSON stdio IPC

---

This plan assumes the existing subsystem and CI plans remain the design baseline. It focuses on the missing product-completion work in the current repository state: protocol expansion, plugin loops, orchestrator wiring, host UI state, and local end-to-end verification.

## File Structure

- `crates/contracts/src/capture.rs`: shared capture source and frame descriptor types.
- `crates/contracts/src/control.rs`: shared control capability, setup checklist, and control session types.
- `crates/contracts/src/grounding.rs`: shared grounding request/result types.
- `crates/contracts/src/plugin.rs`: plugin descriptor and plugin kind metadata.
- `crates/plugin-protocol/Cargo.toml`: protocol crate dependencies on shared contracts.
- `crates/plugin-protocol/src/lib.rs`: all host-to-plugin and plugin-to-host request/response messages.
- `crates/plugin-protocol/tests/operations_roundtrip.rs`: protocol serde regression coverage.
- `crates/plugin-runtime/src/lib.rs`: process spawning, handshake, request/response exchange, and clean shutdown.
- `crates/plugin-runtime/tests/plugin_roundtrip.rs`: runtime roundtrip tests against real plugin executables.
- `plugins/control-ble/Cargo.toml`: runtime dependencies for the control plugin executable loop.
- `plugins/control-ble/src/main.rs`: control plugin stdio request loop.
- `plugins/capture-window/Cargo.toml`: runtime dependencies for the window-capture plugin loop.
- `plugins/capture-window/src/main.rs`: window-capture plugin stdio request loop.
- `plugins/capture-direct/Cargo.toml`: runtime dependencies for the direct-capture plugin loop.
- `plugins/capture-direct/src/main.rs`: direct-capture plugin stdio request loop.
- `plugins/grounding-core/Cargo.toml`: runtime dependencies for the grounding plugin executable loop.
- `plugins/grounding-core/src/main.rs`: grounding plugin stdio request loop.
- `crates/capability-registry/src/lib.rs`: host capability snapshots used by the UI and orchestrator.
- `crates/device-registry/src/lib.rs`: known-device records and preferred plugin choices.
- `crates/telemetry-store/src/lib.rs`: telemetry collection and query helpers.
- `crates/session-orchestrator/Cargo.toml`: orchestrator dependencies on contracts, runtime, registries, and telemetry.
- `crates/session-orchestrator/src/lib.rs`: host-side orchestration of plugin handshakes, probes, session startup, and view state.
- `crates/session-orchestrator/tests/mock_flow.rs`: orchestrator integration coverage with mock-backed plugins.
- `apps/host-desktop/Cargo.toml`: host shell dependencies on the orchestrator layer.
- `apps/host-desktop/src/lib.rs`: host module exports.
- `apps/host-desktop/src/main.rs`: desktop shell startup entry point.
- `apps/host-desktop/src/app.rs`: app state, panel routing, and demo startup wiring.
- `apps/host-desktop/src/view_models/dashboard.rs`: dashboard summary model.
- `apps/host-desktop/src/view_models/device_detail.rs`: per-device details and setup checklist model.
- `apps/host-desktop/src/view_models/session.rs`: capture frame and selected source model.
- `apps/host-desktop/src/view_models/diagnostics.rs`: control and grounding diagnostics model.
- `apps/host-desktop/src/view_models/settings.rs`: capability and plugin settings model.
- `apps/host-desktop/src/panels/dashboard.rs`: dashboard rendering.
- `apps/host-desktop/src/panels/device_detail.rs`: capture source and control setup rendering.
- `apps/host-desktop/src/panels/session_view.rs`: active session rendering.
- `apps/host-desktop/src/panels/diagnostics.rs`: diagnostics rendering.
- `apps/host-desktop/src/panels/settings.rs`: capability and plugin settings rendering.
- `apps/host-desktop/tests/app_state.rs`: host state and panel rendering tests.
- `README.md`: end-to-end developer usage instructions aligned with the finished runtime.

### Task 1: Expand Shared Contracts And The Plugin Protocol

**Files:**
- Modify: `crates/contracts/src/capture.rs`
- Modify: `crates/contracts/src/control.rs`
- Modify: `crates/contracts/src/grounding.rs`
- Modify: `crates/contracts/src/plugin.rs`
- Modify: `crates/plugin-protocol/Cargo.toml`
- Modify: `crates/plugin-protocol/src/lib.rs`
- Create: `crates/plugin-protocol/tests/operations_roundtrip.rs`

- [ ] **Step 1: Write the failing test**

```rust
use ios_control_contracts::grounding::{GroundingPlan, GroundingRequest, PlanKind, TargetInput};
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};

#[test]
fn host_to_plugin_roundtrips_operational_messages() {
    let request = HostToPlugin::PlanGrounding {
        request: GroundingRequest {
            target: TargetInput {
                semantic_label: Some("Settings".into()),
                visual_region: Some((20, 20, 120, 44)),
                confidence: 0.94,
            },
            device_size: (1179, 2556),
            pointer_estimate: (60.0, 40.0),
            uncertainty_radius: 8.0,
            focus_confidence: 0.75,
            keyboard_preferred: false,
        },
    };

    let json = serde_json::to_string(&request).unwrap();
    let decoded: HostToPlugin = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, request);

    let response = PluginToHost::GroundingPlan {
        plan: GroundingPlan {
            kind: PlanKind::Pointer,
            failure: None,
            summary: "tap target".into(),
        },
    };

    let response_json = serde_json::to_string(&response).unwrap();
    let decoded_response: PluginToHost = serde_json::from_str(&response_json).unwrap();
    assert_eq!(decoded_response, response);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ios-control-plugin-protocol host_to_plugin_roundtrips_operational_messages -- --exact`
Expected: FAIL with unresolved imports or missing `PlanGrounding`, `GroundingRequest`, or `GroundingPlan` definitions.

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/contracts/src/capture.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    Window,
    DirectReceiver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameHealth {
    Healthy,
    Occluded,
    Stalled,
    Resized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoSource {
    pub source_id: String,
    pub display_name: String,
    pub kind: SourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoFrameDescriptor {
    pub source_id: String,
    pub source_kind: SourceKind,
    pub width: u32,
    pub height: u32,
    pub rotation_degrees: u16,
    pub frame_index: u64,
    pub health: FrameHealth,
}
```

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlCapability {
    pub supported: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlSetupChecklist {
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlSessionPhase {
    Unavailable,
    ReadyToAdvertise,
    Advertising,
    Connected,
    Error,
}
```

```rust
// crates/contracts/src/grounding.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetInput {
    pub semantic_label: Option<String>,
    pub visual_region: Option<(u32, u32, u32, u32)>,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundingRequest {
    pub target: TargetInput,
    pub device_size: (u32, u32),
    pub pointer_estimate: (f32, f32),
    pub uncertainty_radius: f32,
    pub focus_confidence: f32,
    pub keyboard_preferred: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundingPlan {
    pub kind: PlanKind,
    pub failure: Option<GroundingFailure>,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanKind {
    Pointer,
    Keyboard,
    Hybrid,
}

impl PlanKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pointer => "pointer",
            Self::Keyboard => "keyboard",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroundingFailure {
    TargetAmbiguous,
    GeometryUncertain,
    FocusUncertain,
    ExecutionMismatch,
    RecoveryExhausted,
}
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginKind {
    Capture,
    Control,
    Grounding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginDescriptor {
    pub plugin_id: String,
    pub protocol_version: u32,
    pub kind: PluginKind,
    pub display_name: String,
}
```

```rust
// crates/plugin-protocol/src/lib.rs
use ios_control_contracts::capture::{VideoFrameDescriptor, VideoSource};
use ios_control_contracts::control::{ControlCapability, ControlSessionPhase, ControlSetupChecklist};
use ios_control_contracts::grounding::{GroundingPlan, GroundingRequest};
use ios_control_contracts::plugin::PluginDescriptor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HostToPlugin {
    Handshake { protocol_version: u32 },
    ProbeControl,
    PrepareControl,
    ListCaptureSources,
    GetCaptureFrame { source_id: String },
    StartDirectCapture,
    PlanGrounding { request: GroundingRequest },
    Stop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginToHost {
    HandshakeAck { descriptor: PluginDescriptor },
    ControlCapability { capability: ControlCapability },
    ControlSession {
        phase: ControlSessionPhase,
        checklist: ControlSetupChecklist,
    },
    CaptureSources { sources: Vec<VideoSource> },
    CaptureFrame { frame: VideoFrameDescriptor },
    GroundingPlan { plan: GroundingPlan },
    Ack,
    Error { message: String },
}
```

```toml
# crates/plugin-protocol/Cargo.toml
[package]
name = "ios-control-plugin-protocol"
version = "0.1.0"
edition = "2021"

[dependencies]
ios-control-contracts = { path = "../contracts" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ios-control-plugin-protocol host_to_plugin_roundtrips_operational_messages -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/contracts/src/capture.rs crates/contracts/src/control.rs crates/contracts/src/grounding.rs crates/contracts/src/plugin.rs crates/plugin-protocol/Cargo.toml crates/plugin-protocol/src/lib.rs crates/plugin-protocol/tests/operations_roundtrip.rs
git commit -m "feat: expand shared protocol for end-to-end runtime"
```

### Task 2: Implement Stateful Plugin Executables And Runtime Roundtrips

**Files:**
- Modify: `crates/plugin-runtime/src/lib.rs`
- Create: `crates/plugin-runtime/tests/plugin_roundtrip.rs`
- Modify: `plugins/control-ble/Cargo.toml`
- Modify: `plugins/control-ble/src/main.rs`
- Modify: `plugins/capture-window/Cargo.toml`
- Modify: `plugins/capture-window/src/main.rs`
- Modify: `plugins/capture-direct/Cargo.toml`
- Modify: `plugins/capture-direct/src/main.rs`
- Modify: `plugins/grounding-core/Cargo.toml`
- Modify: `plugins/grounding-core/src/main.rs`

- [ ] **Step 1: Write the failing test**

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use ios_control_plugin_runtime::RunningPlugin;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

#[tokio::test]
async fn plugin_runtime_roundtrips_with_real_plugins() {
    let workspace_root = workspace_root();
    let status = Command::new("cargo")
        .args(["build", "-p", "plugin-capture-window", "-p", "plugin-control-ble", "-p", "plugin-grounding-core"])
        .current_dir(&workspace_root)
        .status()
        .unwrap();
    assert!(status.success());

    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let capture_path = workspace_root.join(format!("target/debug/plugin-capture-window{exe_suffix}"));
    let control_path = workspace_root.join(format!("target/debug/plugin-control-ble{exe_suffix}"));

    let mut capture = RunningPlugin::spawn(&capture_path).await.unwrap();
    let _ = capture.handshake().await.unwrap();
    let sources = capture
        .request(HostToPlugin::ListCaptureSources)
        .await
        .unwrap();
    match sources {
        PluginToHost::CaptureSources { sources } => assert_eq!(sources[0].source_id, "window:mock"),
        other => panic!("unexpected response: {other:?}"),
    }

    let mut control = RunningPlugin::spawn(&control_path).await.unwrap();
    let _ = control.handshake().await.unwrap();
    let prepared = control.request(HostToPlugin::PrepareControl).await.unwrap();
    match prepared {
        PluginToHost::ControlSession { checklist, .. } => {
            assert!(checklist.items.iter().any(|item| item.contains("AssistiveTouch")));
        }
        other => panic!("unexpected response: {other:?}"),
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ios-control-plugin-runtime plugin_runtime_roundtrips_with_real_plugins -- --exact`
Expected: FAIL because `RunningPlugin` or `request()` does not exist and plugin binaries do not emit protocol JSON.

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/plugin-runtime/src/lib.rs
use std::path::Path;
use std::process::Stdio;

use anyhow::{anyhow, Result};
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

pub struct RunningPlugin {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl RunningPlugin {
    pub async fn spawn(plugin_path: &Path) -> Result<Self> {
        let mut child = Command::new(plugin_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| anyhow!("missing stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("missing stdout"))?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    pub async fn handshake(&mut self) -> Result<PluginToHost> {
        self.request(HostToPlugin::Handshake { protocol_version: 1 }).await
    }

    pub async fn request(&mut self, message: HostToPlugin) -> Result<PluginToHost> {
        let json = serde_json::to_string(&message)?;
        self.stdin.write_all(json.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;

        let mut line = String::new();
        let read = self.stdout.read_line(&mut line).await?;
        if read == 0 {
            return Err(anyhow!("plugin closed stdout"));
        }

        Ok(serde_json::from_str(line.trim_end())?)
    }

    pub async fn stop(mut self) -> Result<()> {
        let _ = self.request(HostToPlugin::Stop).await;
        let _ = self.child.wait().await?;
        Ok(())
    }
}
```

```toml
# plugins/control-ble/Cargo.toml
[package]
name = "plugin-control-ble"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
ios-control-contracts = { path = "../../crates/contracts" }
ios-control-plugin-protocol = { path = "../../crates/plugin-protocol" }
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "io-util", "io-std"] }
```

```rust
// plugins/control-ble/src/main.rs
use ios_control_contracts::control::{ControlCapability, ControlSessionPhase, ControlSetupChecklist};
use ios_control_contracts::plugin::{PluginDescriptor, PluginKind};
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut lines = BufReader::new(io::stdin()).lines();
    let mut stdout = io::stdout();

    while let Some(line) = lines.next_line().await? {
        let reply = match serde_json::from_str::<HostToPlugin>(&line)? {
            HostToPlugin::Handshake { .. } => PluginToHost::HandshakeAck {
                descriptor: PluginDescriptor {
                    plugin_id: "control.ble".into(),
                    protocol_version: 1,
                    kind: PluginKind::Control,
                    display_name: "Bluetooth Control".into(),
                },
            },
            HostToPlugin::ProbeControl => PluginToHost::ControlCapability {
                capability: ControlCapability {
                    supported: true,
                    reason: None,
                },
            },
            HostToPlugin::PrepareControl => PluginToHost::ControlSession {
                phase: ControlSessionPhase::ReadyToAdvertise,
                checklist: ControlSetupChecklist {
                    items: vec![
                        "Enable AssistiveTouch on the iPhone or iPad".into(),
                        "Enable Full Keyboard Access for keyboard navigation".into(),
                        "Pair the host over Bluetooth".into(),
                    ],
                },
            },
            HostToPlugin::Stop => {
                stdout.write_all(serde_json::to_string(&PluginToHost::Ack)?.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                break;
            }
            _ => PluginToHost::Error {
                message: "unsupported request for control plugin".into(),
            },
        };

        stdout.write_all(serde_json::to_string(&reply)?.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
    }

    Ok(())
}
```

```rust
// plugins/capture-window/src/main.rs
use ios_control_contracts::capture::VideoSource;
use ios_control_contracts::plugin::{PluginDescriptor, PluginKind};
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use plugin_capture_window::backend::mock_frame;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut lines = BufReader::new(io::stdin()).lines();
    let mut stdout = io::stdout();

    while let Some(line) = lines.next_line().await? {
        let reply = match serde_json::from_str::<HostToPlugin>(&line)? {
            HostToPlugin::Handshake { .. } => PluginToHost::HandshakeAck {
                descriptor: PluginDescriptor {
                    plugin_id: "capture.window".into(),
                    protocol_version: 1,
                    kind: PluginKind::Capture,
                    display_name: "Window Capture".into(),
                },
            },
            HostToPlugin::ListCaptureSources => PluginToHost::CaptureSources {
                sources: vec![VideoSource {
                    source_id: "window:mock".into(),
                    display_name: "Mock iPhone Mirror".into(),
                    kind: ios_control_contracts::capture::SourceKind::Window,
                }],
            },
            HostToPlugin::GetCaptureFrame { source_id } => PluginToHost::CaptureFrame {
                frame: mock_frame(&source_id, 1),
            },
            HostToPlugin::Stop => {
                stdout.write_all(serde_json::to_string(&PluginToHost::Ack)?.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                break;
            }
            _ => PluginToHost::Error {
                message: "unsupported request for capture-window plugin".into(),
            },
        };

        stdout.write_all(serde_json::to_string(&reply)?.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
    }

    Ok(())
}
```

```toml
# plugins/capture-window/Cargo.toml
[package]
name = "plugin-capture-window"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
async-trait = "0.1"
ios-control-contracts = { path = "../../crates/contracts" }
ios-control-plugin-protocol = { path = "../../crates/plugin-protocol" }
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "io-util", "io-std"] }
```

```rust
// plugins/capture-direct/src/main.rs
use ios_control_contracts::plugin::{PluginDescriptor, PluginKind};
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use plugin_capture_direct::backend::first_frame;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut lines = BufReader::new(io::stdin()).lines();
    let mut stdout = io::stdout();

    while let Some(line) = lines.next_line().await? {
        let reply = match serde_json::from_str::<HostToPlugin>(&line)? {
            HostToPlugin::Handshake { .. } => PluginToHost::HandshakeAck {
                descriptor: PluginDescriptor {
                    plugin_id: "capture.direct".into(),
                    protocol_version: 1,
                    kind: PluginKind::Capture,
                    display_name: "Direct Receiver".into(),
                },
            },
            HostToPlugin::StartDirectCapture => PluginToHost::CaptureFrame {
                frame: first_frame("direct:mock"),
            },
            HostToPlugin::Stop => {
                stdout.write_all(serde_json::to_string(&PluginToHost::Ack)?.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                break;
            }
            _ => PluginToHost::Error {
                message: "unsupported request for capture-direct plugin".into(),
            },
        };

        stdout.write_all(serde_json::to_string(&reply)?.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
    }

    Ok(())
}
```

```toml
# plugins/capture-direct/Cargo.toml
[package]
name = "plugin-capture-direct"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
async-trait = "0.1"
ios-control-contracts = { path = "../../crates/contracts" }
ios-control-plugin-protocol = { path = "../../crates/plugin-protocol" }
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "io-util", "io-std"] }
```

```toml
# plugins/grounding-core/Cargo.toml
[package]
name = "plugin-grounding-core"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
ios-control-contracts = { path = "../../crates/contracts" }
ios-control-plugin-protocol = { path = "../../crates/plugin-protocol" }
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "io-util", "io-std"] }
```

```rust
// plugins/grounding-core/src/main.rs
use ios_control_contracts::grounding::{GroundingFailure, GroundingPlan, PlanKind};
use ios_control_contracts::plugin::{PluginDescriptor, PluginKind};
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use plugin_grounding_core::action_selector::ActionSelector;
use plugin_grounding_core::coordinate_mapper::CoordinateMapper;
use plugin_grounding_core::focus_tracker::FocusTracker;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut lines = BufReader::new(io::stdin()).lines();
    let mut stdout = io::stdout();

    while let Some(line) = lines.next_line().await? {
        let reply = match serde_json::from_str::<HostToPlugin>(&line)? {
            HostToPlugin::Handshake { .. } => PluginToHost::HandshakeAck {
                descriptor: PluginDescriptor {
                    plugin_id: "grounding.core".into(),
                    protocol_version: 1,
                    kind: PluginKind::Grounding,
                    display_name: "Grounding Core".into(),
                },
            },
            HostToPlugin::PlanGrounding { request } => {
                let mapper = CoordinateMapper::new(
                    request.device_size,
                    request.pointer_estimate,
                    request.uncertainty_radius,
                );
                let focus = FocusTracker {
                    focus_confidence: request.focus_confidence,
                    keyboard_friendly: request.keyboard_preferred,
                };
                let selector = ActionSelector::default();
                let result = request
                    .target
                    .visual_region
                    .map(|region| mapper.can_confidently_hit(region))
                    .unwrap_or(false);

                match selector.choose_plan(result, &focus, request.uncertainty_radius * 2.0) {
                    Ok(selected) => PluginToHost::GroundingPlan {
                        plan: GroundingPlan {
                            kind: selected.kind,
                            failure: None,
                            summary: format!("selected {}", selected.kind.as_str()),
                        },
                    },
                    Err(failure) => PluginToHost::GroundingPlan {
                        plan: GroundingPlan {
                            kind: PlanKind::Keyboard,
                            failure: Some(match failure {
                                GroundingFailure::TargetAmbiguous => GroundingFailure::TargetAmbiguous,
                                GroundingFailure::GeometryUncertain => GroundingFailure::GeometryUncertain,
                                GroundingFailure::FocusUncertain => GroundingFailure::FocusUncertain,
                                GroundingFailure::ExecutionMismatch => GroundingFailure::ExecutionMismatch,
                                GroundingFailure::RecoveryExhausted => GroundingFailure::RecoveryExhausted,
                            }),
                            summary: "fallback to keyboard".into(),
                        },
                    },
                }
            }
            HostToPlugin::Stop => {
                stdout.write_all(serde_json::to_string(&PluginToHost::Ack)?.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                break;
            }
            _ => PluginToHost::Error {
                message: "unsupported request for grounding plugin".into(),
            },
        };

        stdout.write_all(serde_json::to_string(&reply)?.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
    }

    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ios-control-plugin-runtime plugin_runtime_roundtrips_with_real_plugins -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/plugin-runtime/src/lib.rs crates/plugin-runtime/tests/plugin_roundtrip.rs plugins/control-ble/Cargo.toml plugins/control-ble/src/main.rs plugins/capture-window/Cargo.toml plugins/capture-window/src/main.rs plugins/capture-direct/Cargo.toml plugins/capture-direct/src/main.rs plugins/grounding-core/Cargo.toml plugins/grounding-core/src/main.rs
git commit -m "feat: add stateful plugin runtime roundtrips"
```

### Task 3: Wire Registries, Telemetry, And Session Orchestration

**Files:**
- Modify: `crates/capability-registry/src/lib.rs`
- Modify: `crates/device-registry/src/lib.rs`
- Modify: `crates/telemetry-store/src/lib.rs`
- Modify: `crates/session-orchestrator/Cargo.toml`
- Modify: `crates/session-orchestrator/src/lib.rs`
- Create: `crates/session-orchestrator/tests/mock_flow.rs`

- [ ] **Step 1: Write the failing test**

```rust
use std::path::{Path, PathBuf};

use ios_control_session_orchestrator::{PluginPaths, SessionOrchestrator, StartSessionRequest};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

#[tokio::test]
async fn start_session_collects_mock_plugin_state() {
    let root = workspace_root();
    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let mut orchestrator = SessionOrchestrator::default();

    let state = orchestrator
        .start_session(StartSessionRequest {
            device_id: "device-1".into(),
            device_name: "Mock iPhone".into(),
            selected_source_id: Some("window:mock".into()),
            plugin_paths: PluginPaths {
                capture: root.join(format!("target/debug/plugin-capture-window{exe_suffix}")),
                control: root.join(format!("target/debug/plugin-control-ble{exe_suffix}")),
                grounding: Some(root.join(format!("target/debug/plugin-grounding-core{exe_suffix}"))),
            },
        })
        .await
        .unwrap();

    assert_eq!(state.summary.device_name, "Mock iPhone");
    assert_eq!(state.capture_sources[0].source_id, "window:mock");
    assert_eq!(state.control_checklist.items.len(), 3);
    assert!(state.latest_frame.is_some());
    assert!(state.diagnostics.grounding_summary.contains("selected"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ios-control-session-orchestrator start_session_collects_mock_plugin_state -- --exact`
Expected: FAIL because `PluginPaths`, `StartSessionRequest`, or the richer session state does not exist.

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/capability-registry/src/lib.rs
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySnapshot {
    pub supported: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CapabilityRegistry {
    entries: BTreeMap<String, CapabilitySnapshot>,
}

impl CapabilityRegistry {
    pub fn record(&mut self, key: impl Into<String>, supported: bool, reason: Option<String>) {
        self.entries
            .insert(key.into(), CapabilitySnapshot { supported, reason });
    }

    pub fn get(&self, key: &str) -> Option<&CapabilitySnapshot> {
        self.entries.get(key)
    }
}
```

```rust
// crates/device-registry/src/lib.rs
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRecord {
    pub device_id: String,
    pub device_name: String,
    pub preferred_capture_plugin: String,
    pub preferred_control_plugin: String,
    pub preferred_grounding_plugin: Option<String>,
    pub last_source_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DeviceRegistry {
    entries: BTreeMap<String, DeviceRecord>,
}

impl DeviceRegistry {
    pub fn upsert(&mut self, record: DeviceRecord) {
        self.entries.insert(record.device_id.clone(), record);
    }

    pub fn get(&self, device_id: &str) -> Option<&DeviceRecord> {
        self.entries.get(device_id)
    }
}
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

    pub fn for_session(&self, session_id: &str) -> Vec<TelemetryEvent> {
        self.events
            .iter()
            .filter(|event| event.session_id == session_id)
            .cloned()
            .collect()
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
ios-control-capability-registry = { path = "../capability-registry" }
ios-control-contracts = { path = "../contracts" }
ios-control-device-registry = { path = "../device-registry" }
ios-control-plugin-protocol = { path = "../plugin-protocol" }
ios-control-plugin-runtime = { path = "../plugin-runtime" }
ios-control-telemetry-store = { path = "../telemetry-store" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
// crates/session-orchestrator/src/lib.rs
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use ios_control_capability_registry::CapabilityRegistry;
use ios_control_contracts::capture::{VideoFrameDescriptor, VideoSource};
use ios_control_contracts::control::ControlSetupChecklist;
use ios_control_contracts::grounding::{GroundingRequest, TargetInput};
use ios_control_contracts::plugin::PluginHealth;
use ios_control_contracts::session::{DeviceSessionSummary, SessionPhase};
use ios_control_device_registry::{DeviceRecord, DeviceRegistry};
use ios_control_plugin_protocol::{HostToPlugin, PluginToHost};
use ios_control_plugin_runtime::RunningPlugin;
use ios_control_telemetry_store::{TelemetryEvent, TelemetryStore};

#[derive(Debug, Clone)]
pub struct PluginPaths {
    pub capture: PathBuf,
    pub control: PathBuf,
    pub grounding: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct StartSessionRequest {
    pub device_id: String,
    pub device_name: String,
    pub selected_source_id: Option<String>,
    pub plugin_paths: PluginPaths,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionDiagnostics {
    pub control_summary: String,
    pub grounding_summary: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveSessionState {
    pub summary: DeviceSessionSummary,
    pub capture_sources: Vec<VideoSource>,
    pub latest_frame: Option<VideoFrameDescriptor>,
    pub control_checklist: ControlSetupChecklist,
    pub diagnostics: SessionDiagnostics,
}

#[derive(Debug, Default)]
pub struct SessionOrchestrator {
    pub capabilities: CapabilityRegistry,
    pub devices: DeviceRegistry,
    pub telemetry: TelemetryStore,
}

impl SessionOrchestrator {
    pub async fn start_session(&mut self, request: StartSessionRequest) -> Result<ActiveSessionState> {
        let mut capture = RunningPlugin::spawn(&request.plugin_paths.capture).await?;
        let mut control = RunningPlugin::spawn(&request.plugin_paths.control).await?;

        let _ = capture.handshake().await?;
        let _ = control.handshake().await?;

        let capture_sources = match capture.request(HostToPlugin::ListCaptureSources).await? {
            PluginToHost::CaptureSources { sources } => sources,
            other => return Err(anyhow!("unexpected capture sources response: {other:?}")),
        };

        let source_id = request
            .selected_source_id
            .clone()
            .or_else(|| capture_sources.first().map(|source| source.source_id.clone()))
            .ok_or_else(|| anyhow!("no capture sources available"))?;

        let latest_frame = match capture
            .request(HostToPlugin::GetCaptureFrame {
                source_id: source_id.clone(),
            })
            .await?
        {
            PluginToHost::CaptureFrame { frame } => Some(frame),
            other => return Err(anyhow!("unexpected frame response: {other:?}")),
        };

        let control_checklist = match control.request(HostToPlugin::PrepareControl).await? {
            PluginToHost::ControlSession { checklist, .. } => checklist,
            other => return Err(anyhow!("unexpected control response: {other:?}")),
        };

        self.capabilities.record("control.ble", true, None);

        let grounding_summary = if let Some(path) = &request.plugin_paths.grounding {
            let mut grounding = RunningPlugin::spawn(path).await?;
            let _ = grounding.handshake().await?;
            match grounding
                .request(HostToPlugin::PlanGrounding {
                    request: GroundingRequest {
                        target: TargetInput {
                            semantic_label: Some("Settings".into()),
                            visual_region: Some((20, 20, 120, 44)),
                            confidence: 0.94,
                        },
                        device_size: (1179, 2556),
                        pointer_estimate: (60.0, 40.0),
                        uncertainty_radius: 8.0,
                        focus_confidence: 0.75,
                        keyboard_preferred: false,
                    },
                })
                .await?
            {
                PluginToHost::GroundingPlan { plan } => plan.summary,
                other => return Err(anyhow!("unexpected grounding response: {other:?}")),
            }
        } else {
            "grounding disabled".into()
        };

        let summary = DeviceSessionSummary {
            device_id: request.device_id.clone(),
            device_name: request.device_name.clone(),
            phase: SessionPhase::Streaming,
            plugin_health: PluginHealth::Healthy,
            capture_plugin: Some("capture.window".into()),
            control_plugin: Some("control.ble".into()),
            grounding_plugin: request.plugin_paths.grounding.as_ref().map(|_| "grounding.core".into()),
        };

        self.devices.upsert(DeviceRecord {
            device_id: request.device_id.clone(),
            device_name: request.device_name.clone(),
            preferred_capture_plugin: "capture.window".into(),
            preferred_control_plugin: "control.ble".into(),
            preferred_grounding_plugin: request.plugin_paths.grounding.as_ref().map(|_| "grounding.core".into()),
            last_source_id: Some(source_id),
        });

        self.telemetry.push(TelemetryEvent {
            session_id: request.device_id.clone(),
            message: "session started".into(),
        });

        Ok(ActiveSessionState {
            summary,
            capture_sources,
            latest_frame,
            control_checklist,
            diagnostics: SessionDiagnostics {
                control_summary: "control ready".into(),
                grounding_summary,
            },
        })
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ios-control-session-orchestrator start_session_collects_mock_plugin_state -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/capability-registry/src/lib.rs crates/device-registry/src/lib.rs crates/telemetry-store/src/lib.rs crates/session-orchestrator/Cargo.toml crates/session-orchestrator/src/lib.rs crates/session-orchestrator/tests/mock_flow.rs
git commit -m "feat: wire host session orchestration across plugins"
```

### Task 4: Connect The Desktop Shell To Session State

**Files:**
- Modify: `apps/host-desktop/Cargo.toml`
- Modify: `apps/host-desktop/src/lib.rs`
- Modify: `apps/host-desktop/src/main.rs`
- Modify: `apps/host-desktop/src/app.rs`
- Create: `apps/host-desktop/src/view_models/device_detail.rs`
- Create: `apps/host-desktop/src/view_models/session.rs`
- Create: `apps/host-desktop/src/view_models/diagnostics.rs`
- Create: `apps/host-desktop/src/view_models/settings.rs`
- Modify: `apps/host-desktop/src/panels/device_detail.rs`
- Modify: `apps/host-desktop/src/panels/session_view.rs`
- Modify: `apps/host-desktop/src/panels/settings.rs`
- Create: `apps/host-desktop/tests/app_state.rs`

- [ ] **Step 1: Write the failing test**

```rust
use host_desktop::app::HostDesktopApp;
use host_desktop::view_models::dashboard::DashboardViewModel;
use host_desktop::view_models::device_detail::DeviceDetailViewModel;
use host_desktop::view_models::diagnostics::DiagnosticsViewModel;
use host_desktop::view_models::session::SessionViewModel;
use host_desktop::view_models::settings::SettingsViewModel;

#[test]
fn host_app_exposes_end_to_end_demo_state() {
    let app = HostDesktopApp::demo();

    assert_eq!(app.dashboard.total_devices, 1);
    assert_eq!(app.dashboard.degraded_devices, 0);
    assert_eq!(app.device_detail.device_name, "Mock iPhone");
    assert_eq!(app.session.selected_source_label, "Window: Mock iPhone Mirror");
    assert!(app.diagnostics.grounding_summary.contains("selected"));
    assert!(app.settings.plugin_rows.iter().any(|row| row.contains("control.ble")));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p host-desktop host_app_exposes_end_to_end_demo_state -- --exact`
Expected: FAIL because `HostDesktopApp::demo()` and the new view-model modules do not exist.

- [ ] **Step 3: Write minimal implementation**

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
ios-control-session-orchestrator = { path = "../../crates/session-orchestrator" }
```

```rust
// apps/host-desktop/src/view_models/device_detail.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDetailViewModel {
    pub device_name: String,
    pub capture_source_labels: Vec<String>,
    pub control_checklist: Vec<String>,
}
```

```rust
// apps/host-desktop/src/view_models/session.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionViewModel {
    pub selected_source_label: String,
    pub frame_summary: String,
}
```

```rust
// apps/host-desktop/src/view_models/diagnostics.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsViewModel {
    pub control_summary: String,
    pub grounding_summary: String,
}
```

```rust
// apps/host-desktop/src/view_models/settings.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsViewModel {
    pub plugin_rows: Vec<String>,
}
```

```rust
// apps/host-desktop/src/lib.rs
pub mod app;
pub mod panels {
    pub mod dashboard;
    pub mod device_detail;
    pub mod diagnostics;
    pub mod session_view;
    pub mod settings;
}
pub mod view_models {
    pub mod dashboard;
    pub mod device_detail;
    pub mod diagnostics;
    pub mod session;
    pub mod settings;
}
```

```rust
// apps/host-desktop/src/app.rs
use eframe::egui;

use crate::panels::{dashboard, device_detail, diagnostics, session_view, settings};
use crate::view_models::dashboard::DashboardViewModel;
use crate::view_models::device_detail::DeviceDetailViewModel;
use crate::view_models::diagnostics::DiagnosticsViewModel;
use crate::view_models::session::SessionViewModel;
use crate::view_models::settings::SettingsViewModel;

pub struct HostDesktopApp {
    pub dashboard: DashboardViewModel,
    pub device_detail: DeviceDetailViewModel,
    pub session: SessionViewModel,
    pub diagnostics: DiagnosticsViewModel,
    pub settings: SettingsViewModel,
}

impl HostDesktopApp {
    pub fn demo() -> Self {
        Self {
            dashboard: DashboardViewModel {
                total_devices: 1,
                degraded_devices: 0,
            },
            device_detail: DeviceDetailViewModel {
                device_name: "Mock iPhone".into(),
                capture_source_labels: vec!["Window: Mock iPhone Mirror".into()],
                control_checklist: vec![
                    "Enable AssistiveTouch on the iPhone or iPad".into(),
                    "Enable Full Keyboard Access for keyboard navigation".into(),
                    "Pair the host over Bluetooth".into(),
                ],
            },
            session: SessionViewModel {
                selected_source_label: "Window: Mock iPhone Mirror".into(),
                frame_summary: "1280x720 frame 1".into(),
            },
            diagnostics: DiagnosticsViewModel {
                control_summary: "control ready".into(),
                grounding_summary: "selected pointer".into(),
            },
            settings: SettingsViewModel {
                plugin_rows: vec![
                    "capture.window".into(),
                    "control.ble".into(),
                    "grounding.core".into(),
                ],
            },
        }
    }
}

impl eframe::App for HostDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            dashboard::render(ui, &self.dashboard);
            ui.separator();
            device_detail::render(
                ui,
                &self.device_detail.device_name,
                &self.device_detail.capture_source_labels,
                &self.device_detail.control_checklist,
            );
            ui.separator();
            session_view::render_summary(ui, &self.session.frame_summary, &self.session.selected_source_label);
            ui.separator();
            diagnostics::render(ui, &self.diagnostics.grounding_summary);
            diagnostics::render_control_diagnostics(ui, &self.diagnostics.control_summary);
            ui.separator();
            settings::render_rows(ui, &self.settings.plugin_rows);
        });
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
        Box::new(|_cc| Ok(Box::new(host_desktop::app::HostDesktopApp::demo()))),
    )
}
```

```rust
// apps/host-desktop/src/panels/device_detail.rs
use egui::Ui;

pub fn render(ui: &mut Ui, device_name: &str, capture_source_labels: &[String], control_checklist: &[String]) {
    ui.heading("Device Detail");
    ui.label(device_name);
    for label in capture_source_labels {
        ui.label(label);
    }
    for item in control_checklist {
        ui.label(item);
    }
}
```

```rust
// apps/host-desktop/src/panels/session_view.rs
use egui::Ui;

pub fn render_summary(ui: &mut Ui, frame_summary: &str, source_label: &str) {
    ui.heading("Session View");
    ui.label(source_label);
    ui.label(frame_summary);
}
```

```rust
// apps/host-desktop/src/panels/settings.rs
use egui::Ui;

pub fn render_rows(ui: &mut Ui, rows: &[String]) {
    ui.heading("Settings");
    for row in rows {
        ui.label(row);
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p host-desktop host_app_exposes_end_to_end_demo_state -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/host-desktop/Cargo.toml apps/host-desktop/src/lib.rs apps/host-desktop/src/main.rs apps/host-desktop/src/app.rs apps/host-desktop/src/view_models/device_detail.rs apps/host-desktop/src/view_models/session.rs apps/host-desktop/src/view_models/diagnostics.rs apps/host-desktop/src/view_models/settings.rs apps/host-desktop/src/panels/device_detail.rs apps/host-desktop/src/panels/session_view.rs apps/host-desktop/src/panels/settings.rs apps/host-desktop/tests/app_state.rs
git commit -m "feat: connect desktop shell to end-to-end session state"
```

### Task 5: Add Mock End-To-End Verification And Final Usage Docs

**Files:**
- Create: `crates/session-orchestrator/tests/local_mock_e2e.rs`
- Modify: `README.md`

- [ ] **Step 1: Write the failing test**

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

use ios_control_contracts::session::SessionPhase;
use ios_control_session_orchestrator::{PluginPaths, SessionOrchestrator, StartSessionRequest};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

#[tokio::test]
async fn local_mock_e2e_builds_streaming_session() {
    let root = workspace_root();
    let build = Command::new("cargo")
        .args([
            "build",
            "-p",
            "plugin-capture-window",
            "-p",
            "plugin-control-ble",
            "-p",
            "plugin-grounding-core",
        ])
        .current_dir(&root)
        .status()
        .unwrap();
    assert!(build.success());

    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let mut orchestrator = SessionOrchestrator::default();
    let state = orchestrator
        .start_session(StartSessionRequest {
            device_id: "device-e2e".into(),
            device_name: "Mock iPhone".into(),
            selected_source_id: Some("window:mock".into()),
            plugin_paths: PluginPaths {
                capture: root.join(format!("target/debug/plugin-capture-window{exe_suffix}")),
                control: root.join(format!("target/debug/plugin-control-ble{exe_suffix}")),
                grounding: Some(root.join(format!("target/debug/plugin-grounding-core{exe_suffix}"))),
            },
        })
        .await
        .unwrap();

    assert_eq!(state.summary.phase, SessionPhase::Streaming);
    assert_eq!(state.capture_sources.len(), 1);
    assert_eq!(state.control_checklist.items.len(), 3);
    assert!(state.latest_frame.is_some());
    assert!(state.diagnostics.control_summary.contains("ready"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ios-control-session-orchestrator local_mock_e2e_builds_streaming_session -- --exact`
Expected: FAIL because the local orchestrator path is not yet stable enough to pass the full mock-backed flow.

- [ ] **Step 3: Write minimal implementation**

````markdown
# README.md
## End-to-end developer flow

```bash
cargo build -p plugin-capture-window -p plugin-control-ble -p plugin-grounding-core
cargo test -p ios-control-session-orchestrator local_mock_e2e_builds_streaming_session -- --exact
cargo run -p host-desktop
```

Expected result:

- plugin binaries build successfully
- the orchestrator integration test reports a streaming mock session
- the host shell opens with the mock-backed demo state
````

```rust
// crates/session-orchestrator/tests/local_mock_e2e.rs
use std::path::{Path, PathBuf};
use std::process::Command;

use ios_control_contracts::session::SessionPhase;
use ios_control_session_orchestrator::{PluginPaths, SessionOrchestrator, StartSessionRequest};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

#[tokio::test]
async fn local_mock_e2e_builds_streaming_session() {
    let root = workspace_root();
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            "plugin-capture-window",
            "-p",
            "plugin-control-ble",
            "-p",
            "plugin-grounding-core",
        ])
        .current_dir(&root)
        .status()
        .unwrap();
    assert!(status.success());

    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let mut orchestrator = SessionOrchestrator::default();
    let state = orchestrator
        .start_session(StartSessionRequest {
            device_id: "device-e2e".into(),
            device_name: "Mock iPhone".into(),
            selected_source_id: Some("window:mock".into()),
            plugin_paths: PluginPaths {
                capture: root.join(format!("target/debug/plugin-capture-window{exe_suffix}")),
                control: root.join(format!("target/debug/plugin-control-ble{exe_suffix}")),
                grounding: Some(root.join(format!("target/debug/plugin-grounding-core{exe_suffix}"))),
            },
        })
        .await
        .unwrap();

    assert_eq!(state.summary.phase, SessionPhase::Streaming);
    assert_eq!(state.capture_sources[0].source_id, "window:mock");
    assert_eq!(state.control_checklist.items.len(), 3);
    assert!(state.latest_frame.is_some());
    assert!(state.diagnostics.grounding_summary.contains("selected"));
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p ios-control-session-orchestrator local_mock_e2e_builds_streaming_session -- --exact
cargo test --workspace
python3 -m unittest discover -s tests/ci -p 'test_*.py' -v
python3 scripts/assert_ci_release.py full
```

Expected: all commands pass; the targeted E2E test reports a streaming session, the workspace tests end with `ok`, the CI unittests end with `OK`, and the workflow assertion exits `0` with no output.

- [ ] **Step 5: Commit**

```bash
git add crates/session-orchestrator/tests/local_mock_e2e.rs README.md
git commit -m "feat: add mock end-to-end verification flow"
```
