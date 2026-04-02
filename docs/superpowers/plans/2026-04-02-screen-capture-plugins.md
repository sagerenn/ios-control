# iOS Screen Capture Plugins Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the capture subsystem so the host app can consume frames from either a third-party mirroring window or a direct-receiver compatibility backend through one normalized API.

**Architecture:** Extend the shared contracts with a `VideoSource` model and use shared-memory frame descriptors so large frames do not travel through JSON IPC. Implement two plugin binaries, `plugin-capture-window` and `plugin-capture-direct`, with the host app selecting sources through the session orchestrator and rendering health state in the device/session views.

**Tech Stack:** Rust, tokio, serde, memmap2, windows crate, zbus/portal bindings, tracing

---

**Prerequisite:** Execute [2026-04-02-host-app-foundation-linux-windows.md](/home/ubuntu/ios-control/docs/superpowers/plans/2026-04-02-host-app-foundation-linux-windows.md) first so the workspace, plugin runtime, and desktop shell already exist.

## File Structure

- `crates/contracts/src/capture.rs`: normalized capture types used by shell and plugins.
- `crates/frame-transport/src/lib.rs`: shared-memory frame descriptor and ring-buffer helpers.
- `plugins/capture-window/src/lib.rs`: window-capture plugin exports used by tests.
- `plugins/capture-window/src/main.rs`: window ingestion plugin entrypoint.
- `plugins/capture-window/src/backend.rs`: capture backend trait and source model.
- `plugins/capture-window/src/windows_backend.rs`: Windows window-capture adapter.
- `plugins/capture-window/src/linux_backend.rs`: Linux portal/compositor adapter.
- `plugins/capture-window/src/mock_backend.rs`: deterministic backend for tests.
- `plugins/capture-direct/src/main.rs`: direct-receiver plugin entrypoint.
- `plugins/capture-direct/src/lib.rs`: direct-receiver plugin exports used by tests.
- `plugins/capture-direct/src/backend.rs`: compatibility-backend trait.
- `plugins/capture-direct/src/helper_launcher.rs`: local helper discovery and process management.
- `plugins/capture-direct/src/mock_backend.rs`: deterministic backend for tests and CI.
- `apps/host-desktop/src/panels/device_detail.rs`: source selection UI.
- `apps/host-desktop/src/panels/session_view.rs`: live preview and source health surface.

### Task 1: Add Capture Contracts And Frame Transport

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/contracts/src/lib.rs`
- Create: `crates/contracts/src/capture.rs`
- Create: `crates/frame-transport/Cargo.toml`
- Create: `crates/frame-transport/src/lib.rs`
- Test: `crates/contracts/tests/capture_contract.rs`

- [ ] **Step 1: Write the failing test**

```rust
use ios_control_contracts::capture::{FrameHealth, SourceKind, VideoFrameDescriptor};

#[test]
fn frame_descriptor_roundtrips_orientation_and_health() {
    let descriptor = VideoFrameDescriptor {
        source_id: "window:airdroid".into(),
        source_kind: SourceKind::Window,
        width: 1179,
        height: 2556,
        rotation_degrees: 90,
        frame_index: 7,
        health: FrameHealth::Occluded,
    };

    let encoded = serde_json::to_string(&descriptor).unwrap();
    let decoded: VideoFrameDescriptor = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded.rotation_degrees, 90);
    assert_eq!(decoded.health, FrameHealth::Occluded);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ios-control-contracts frame_descriptor_roundtrips_orientation_and_health -- --exact`
Expected: FAIL with `could not find 'capture' in 'ios_control_contracts'`

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/contracts/src/lib.rs
pub mod capture;
pub mod plugin;
pub mod session;
```

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
// crates/frame-transport/src/lib.rs
use anyhow::Result;
use memmap2::MmapMut;

pub struct FrameSlot {
    mmap: MmapMut,
}

impl FrameSlot {
    pub fn new(byte_len: usize) -> Result<Self> {
        Ok(Self {
            mmap: MmapMut::map_anon(byte_len)?,
        })
    }

    pub fn write(&mut self, bytes: &[u8]) {
        self.mmap[..bytes.len()].copy_from_slice(bytes);
    }
}
```

```toml
# crates/frame-transport/Cargo.toml
[package]
name = "ios-control-frame-transport"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
memmap2 = "0.9"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ios-control-contracts frame_descriptor_roundtrips_orientation_and_health -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/contracts/src/lib.rs crates/contracts/src/capture.rs crates/contracts/tests/capture_contract.rs crates/frame-transport/Cargo.toml crates/frame-transport/src/lib.rs
git commit -m "feat: add capture contracts and frame transport"
```

### Task 2: Implement The Window Ingestion Plugin

**Files:**
- Modify: `Cargo.toml`
- Create: `plugins/capture-window/Cargo.toml`
- Create: `plugins/capture-window/src/lib.rs`
- Create: `plugins/capture-window/src/main.rs`
- Create: `plugins/capture-window/src/backend.rs`
- Create: `plugins/capture-window/src/mock_backend.rs`
- Create: `plugins/capture-window/src/windows_backend.rs`
- Create: `plugins/capture-window/src/linux_backend.rs`
- Test: `plugins/capture-window/tests/window_contract.rs`

- [ ] **Step 1: Write the failing test**

```rust
use plugin_capture_window::mock_backend::MockWindowBackend;

#[tokio::test]
async fn window_capture_lists_mock_source_then_streams_one_frame() {
    let mut backend = MockWindowBackend::default();
    let sources = backend.list_sources().await.unwrap();

    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].source_id, "window:mock");

    let frame = backend.next_frame("window:mock").await.unwrap();
    assert_eq!(frame.frame_index, 1);
    assert_eq!(frame.width, 1280);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p plugin-capture-window window_capture_lists_mock_source_then_streams_one_frame -- --exact`
Expected: FAIL with `package ID specification 'plugin-capture-window' did not match any packages`

- [ ] **Step 3: Write minimal implementation**

```rust
// plugins/capture-window/src/backend.rs
use async_trait::async_trait;
use ios_control_contracts::capture::{FrameHealth, SourceKind, VideoFrameDescriptor};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSource {
    pub source_id: String,
    pub display_name: String,
}

#[async_trait]
pub trait WindowCaptureBackend {
    async fn list_sources(&mut self) -> anyhow::Result<Vec<WindowSource>>;
    async fn next_frame(&mut self, source_id: &str) -> anyhow::Result<VideoFrameDescriptor>;
}

pub fn mock_frame(source_id: &str, frame_index: u64) -> VideoFrameDescriptor {
    VideoFrameDescriptor {
        source_id: source_id.into(),
        source_kind: SourceKind::Window,
        width: 1280,
        height: 720,
        rotation_degrees: 0,
        frame_index,
        health: FrameHealth::Healthy,
    }
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
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
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
  "crates/frame-transport",
  "apps/host-desktop",
  "plugins/capture-window",
  "plugins/mock-device",
]
resolver = "2"
```

```rust
// plugins/capture-window/src/lib.rs
pub mod backend;
pub mod linux_backend;
pub mod mock_backend;
pub mod windows_backend;
```

```rust
// plugins/capture-window/src/mock_backend.rs
use async_trait::async_trait;

use crate::backend::{mock_frame, WindowCaptureBackend, WindowSource};

#[derive(Default)]
pub struct MockWindowBackend {
    frame_index: u64,
}

#[async_trait]
impl WindowCaptureBackend for MockWindowBackend {
    async fn list_sources(&mut self) -> anyhow::Result<Vec<WindowSource>> {
        Ok(vec![WindowSource {
            source_id: "window:mock".into(),
            display_name: "Mock Mirroring Window".into(),
        }])
    }

    async fn next_frame(&mut self, source_id: &str) -> anyhow::Result<ios_control_contracts::capture::VideoFrameDescriptor> {
        self.frame_index += 1;
        Ok(mock_frame(source_id, self.frame_index))
    }
}
```

```rust
// plugins/capture-window/src/windows_backend.rs
pub fn probe_windows_capture() -> bool {
    cfg!(target_os = "windows")
}
```

```rust
// plugins/capture-window/src/linux_backend.rs
pub fn probe_linux_capture() -> bool {
    cfg!(target_os = "linux")
}
```

```rust
// plugins/capture-window/src/main.rs
fn main() {
    println!("plugin-capture-window");
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p plugin-capture-window window_capture_lists_mock_source_then_streams_one_frame -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml plugins/capture-window/Cargo.toml plugins/capture-window/src/main.rs plugins/capture-window/src/backend.rs plugins/capture-window/src/mock_backend.rs plugins/capture-window/src/windows_backend.rs plugins/capture-window/src/linux_backend.rs plugins/capture-window/tests/window_contract.rs
git commit -m "feat: add window capture plugin"
```

### Task 3: Implement The Direct Receiver Compatibility Plugin

**Files:**
- Modify: `Cargo.toml`
- Create: `plugins/capture-direct/Cargo.toml`
- Create: `plugins/capture-direct/src/lib.rs`
- Create: `plugins/capture-direct/src/main.rs`
- Create: `plugins/capture-direct/src/backend.rs`
- Create: `plugins/capture-direct/src/helper_launcher.rs`
- Create: `plugins/capture-direct/src/mock_backend.rs`
- Test: `plugins/capture-direct/tests/direct_receiver_contract.rs`

- [ ] **Step 1: Write the failing test**

```rust
use plugin_capture_direct::mock_backend::MockDirectReceiverBackend;

#[tokio::test]
async fn direct_receiver_backend_reports_unavailable_without_helper() {
    let backend = MockDirectReceiverBackend::unavailable("helper missing");
    let result = backend.start_session().await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("helper missing"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p plugin-capture-direct direct_receiver_backend_reports_unavailable_without_helper -- --exact`
Expected: FAIL with `package ID specification 'plugin-capture-direct' did not match any packages`

- [ ] **Step 3: Write minimal implementation**

```rust
// plugins/capture-direct/src/backend.rs
use async_trait::async_trait;
use ios_control_contracts::capture::{FrameHealth, SourceKind, VideoFrameDescriptor};

#[async_trait]
pub trait DirectReceiverBackend {
    async fn start_session(&self) -> anyhow::Result<VideoFrameDescriptor>;
}

pub fn first_frame(source_id: &str) -> VideoFrameDescriptor {
    VideoFrameDescriptor {
        source_id: source_id.into(),
        source_kind: SourceKind::DirectReceiver,
        width: 1179,
        height: 2556,
        rotation_degrees: 0,
        frame_index: 1,
        health: FrameHealth::Healthy,
    }
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
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
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
  "crates/frame-transport",
  "apps/host-desktop",
  "plugins/capture-window",
  "plugins/capture-direct",
  "plugins/mock-device",
]
resolver = "2"
```

```rust
// plugins/capture-direct/src/lib.rs
pub mod backend;
pub mod helper_launcher;
pub mod mock_backend;
```

```rust
// plugins/capture-direct/src/helper_launcher.rs
use std::path::PathBuf;

pub fn find_helper() -> Option<PathBuf> {
    std::env::var_os("IOS_CONTROL_DIRECT_RECEIVER_HELPER").map(PathBuf::from)
}
```

```rust
// plugins/capture-direct/src/mock_backend.rs
use async_trait::async_trait;

use crate::backend::{first_frame, DirectReceiverBackend};

pub struct MockDirectReceiverBackend {
    error: Option<String>,
}

impl MockDirectReceiverBackend {
    pub fn unavailable(message: &str) -> Self {
        Self {
            error: Some(message.into()),
        }
    }
}

#[async_trait]
impl DirectReceiverBackend for MockDirectReceiverBackend {
    async fn start_session(&self) -> anyhow::Result<ios_control_contracts::capture::VideoFrameDescriptor> {
        if let Some(message) = &self.error {
            anyhow::bail!(message.clone());
        }

        Ok(first_frame("direct:mock"))
    }
}
```

```rust
// plugins/capture-direct/src/main.rs
fn main() {
    println!("plugin-capture-direct");
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p plugin-capture-direct direct_receiver_backend_reports_unavailable_without_helper -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml plugins/capture-direct/Cargo.toml plugins/capture-direct/src/main.rs plugins/capture-direct/src/backend.rs plugins/capture-direct/src/helper_launcher.rs plugins/capture-direct/src/mock_backend.rs plugins/capture-direct/tests/direct_receiver_contract.rs
git commit -m "feat: add direct receiver plugin shell"
```

### Task 4: Integrate Capture Source Selection Into The Desktop Shell

**Files:**
- Modify: `crates/session-orchestrator/src/lib.rs`
- Modify: `apps/host-desktop/src/panels/device_detail.rs`
- Create: `apps/host-desktop/src/panels/session_view.rs`
- Test: `apps/host-desktop/tests/capture_source_view_model.rs`

- [ ] **Step 1: Write the failing test**

```rust
use host_desktop::panels::device_detail::CaptureSourceOption;

#[test]
fn capture_source_option_labels_window_and_direct_sources() {
    let window = CaptureSourceOption::new("window:airdroid", "AirDroid Window");
    let direct = CaptureSourceOption::new("direct:receiver", "Direct Receiver");

    assert!(window.label().contains("Window"));
    assert!(direct.label().contains("Direct"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p host-desktop capture_source_option_labels_window_and_direct_sources -- --exact`
Expected: FAIL with `no function or associated item named 'new' found for struct 'CaptureSourceOption'`

- [ ] **Step 3: Write minimal implementation**

```rust
// apps/host-desktop/src/panels/device_detail.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSourceOption {
    pub source_id: String,
    pub display_name: String,
}

impl CaptureSourceOption {
    pub fn new(source_id: &str, display_name: &str) -> Self {
        Self {
            source_id: source_id.into(),
            display_name: display_name.into(),
        }
    }

    pub fn label(&self) -> String {
        if self.source_id.starts_with("window:") {
            format!("Window: {}", self.display_name)
        } else {
            format!("Direct: {}", self.display_name)
        }
    }
}
```

```rust
// apps/host-desktop/src/panels/session_view.rs
use egui::Ui;
use ios_control_contracts::capture::VideoFrameDescriptor;

pub fn render(ui: &mut Ui, frame: Option<&VideoFrameDescriptor>) {
    ui.heading("Session View");
    if let Some(frame) = frame {
        ui.label(format!("{}x{} frame {}", frame.width, frame.height, frame.frame_index));
    } else {
        ui.label("No active frame source");
    }
}
```

```rust
// crates/session-orchestrator/src/lib.rs
pub struct CaptureRouting {
    pub selected_source: Option<String>,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p host-desktop capture_source_option_labels_window_and_direct_sources -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/session-orchestrator/src/lib.rs apps/host-desktop/src/panels/device_detail.rs apps/host-desktop/src/panels/session_view.rs apps/host-desktop/tests/capture_source_view_model.rs
git commit -m "feat: integrate capture source selection"
```
