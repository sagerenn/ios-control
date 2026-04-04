# Live Preview Capture Transport Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace synthetic frame-slot fills with actual RGBA frame bytes and render a real preview in the desktop host.

**Architecture:** Keep the helper-backed capture model, but change the helper contract from "metadata plus fill byte" to "metadata plus real RGBA payload". Capture plugins remain responsible for writing bytes into shared frame slots, the orchestrator promotes capture streams to first-class session state, and the host reads slot bytes into egui textures for live preview.

**Tech Stack:** Rust, memmap2, egui textures, `ios-control-frame-transport`, plugin protocol streaming, helper-backed window/direct capture

---

## File Structure

- `crates/frame-transport/src/lib.rs`: add read support for frame slots so the host can consume live RGBA bytes.
- `plugins/capture-window/src/helper_bridge.rs`: parse real RGBA helper events.
- `plugins/capture-window/src/main.rs`: write decoded RGBA bytes into the stream slot.
- `plugins/capture-window/tests/window_contract.rs`: verify that live stream reads produce actual slot bytes.
- `plugins/capture-direct/src/helper_bridge.rs`: parse real RGBA helper events.
- `plugins/capture-direct/src/main.rs`: write decoded RGBA bytes into the stream slot.
- `plugins/capture-direct/tests/direct_receiver_contract.rs`: verify direct-capture slot contents.
- `crates/session-orchestrator/src/lib.rs`: store open capture streams and expose frame refresh.
- `apps/host-desktop/src/preview.rs`: open slot files and convert RGBA bytes into `egui::ColorImage`.
- `apps/host-desktop/src/app.rs`: poll preview frames from the runtime and store them in session state.
- `apps/host-desktop/src/panels/session_view.rs`: render the preview texture instead of only text summaries.
- `apps/host-desktop/tests/support/mod.rs`: helpers for temporary frame-slot files used by preview tests.
- `apps/host-desktop/tests/preview.rs`: host preview regression coverage.

### Task 1: Add Real RGBA Frame Payload Support

**Files:**
- Modify: `crates/frame-transport/Cargo.toml`
- Modify: `crates/frame-transport/src/lib.rs`
- Modify: `plugins/capture-window/src/helper_bridge.rs`
- Modify: `plugins/capture-direct/src/helper_bridge.rs`
- Modify: `plugins/capture-window/tests/window_contract.rs`
- Modify: `plugins/capture-direct/tests/direct_receiver_contract.rs`
- Test: `plugins/capture-window/tests/window_contract.rs`
- Test: `plugins/capture-direct/tests/direct_receiver_contract.rs`

- [ ] **Step 1: Write the failing transport tests**

```rust
#[test]
fn frame_slot_reader_reads_exact_rgba_bytes() {
    let mut slot = FrameSlot::new(8).unwrap();
    slot.write(&[255, 0, 0, 255, 0, 255, 0, 255]).unwrap();

    let reader = FrameSlotReader::open(slot.path(), 8).unwrap();
    assert_eq!(reader.read(), &[255, 0, 0, 255, 0, 255, 0, 255]);
}

#[test]
fn helper_frame_event_decodes_rgba_payload() {
    let event: HelperFrameEvent = serde_json::from_str(
        r#"{"frame_index":1,"width":2,"height":1,"rgba_base64":"ffAA/wD/AP8="}"#,
    )
    .unwrap();

    assert_eq!(event.decode_rgba().unwrap().len(), 8);
}
```

- [ ] **Step 2: Run the transport-focused tests to verify they fail**

Run: `cargo test -p ios-control-frame-transport`

Expected: FAIL because `FrameSlotReader` does not exist yet.

Run: `cargo test -p plugin-capture-window helper_frame_event_decodes_rgba_payload -- --exact`

Expected: FAIL because `HelperFrameEvent` does not expose a decode API or `rgba_base64`.

- [ ] **Step 3: Implement frame-slot reading and RGBA helper decoding**

```rust
pub struct FrameSlotReader {
    file: File,
    mmap: memmap2::Mmap,
    byte_len: usize,
}

impl FrameSlotReader {
    pub fn open(path: &Path, byte_len: usize) -> Result<Self> {
        let file = OpenOptions::new().read(true).open(path)?;
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        Ok(Self { file, mmap, byte_len })
    }

    pub fn read(&self) -> &[u8] {
        &self.mmap[..self.byte_len]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HelperFrameEvent {
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    pub rgba_base64: String,
}

impl HelperFrameEvent {
    pub fn decode_rgba(&self) -> Result<Vec<u8>> {
        base64::engine::general_purpose::STANDARD
            .decode(self.rgba_base64.as_bytes())
            .map_err(Into::into)
    }
}
```

- [ ] **Step 4: Run the focused transport tests to verify they pass**

Run: `cargo test -p ios-control-frame-transport`

Expected: PASS

Run: `cargo test -p plugin-capture-window --test window_contract`

Expected: PASS

Run: `cargo test -p plugin-capture-direct --test direct_receiver_contract`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/frame-transport/Cargo.toml \
  crates/frame-transport/src/lib.rs \
  plugins/capture-window/src/helper_bridge.rs \
  plugins/capture-direct/src/helper_bridge.rs \
  plugins/capture-window/tests/window_contract.rs \
  plugins/capture-direct/tests/direct_receiver_contract.rs
git commit -m "feat: support real rgba payloads in capture helpers"
```

### Task 2: Promote Capture Streams To Live Session State

**Files:**
- Modify: `crates/session-orchestrator/src/lib.rs`
- Modify: `crates/plugin-runtime/tests/plugin_roundtrip.rs`
- Modify: `crates/session-orchestrator/tests/mock_flow.rs`
- Modify: `crates/session-orchestrator/tests/local_mock_e2e.rs`
- Modify: `plugins/capture-window/src/main.rs`
- Modify: `plugins/capture-direct/src/main.rs`
- Test: `crates/plugin-runtime/tests/plugin_roundtrip.rs`
- Test: `crates/session-orchestrator/tests/mock_flow.rs`

- [ ] **Step 1: Write the failing stream-state tests**

```rust
#[tokio::test]
async fn start_session_opens_capture_stream_and_refreshes_frames() {
    let mut orchestrator = SessionOrchestrator::default();
    let mut state = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "device-1".into(),
            device_name: "Mock iPhone".into(),
            selected_source_id: Some("window-helper-1".into()),
            plugin_paths: support::plugin_paths(),
        })
        .await
        .unwrap();

    assert!(state.capture_stream.is_some());
    let previous = state.latest_frame.as_ref().unwrap().frame_index;
    let refreshed = state.refresh_capture_frame().await.unwrap();
    assert!(refreshed.frame_index > previous);
}
```

- [ ] **Step 2: Run the orchestrator test to verify it fails**

Run: `cargo test -p ios-control-session-orchestrator start_session_opens_capture_stream_and_refreshes_frames -- --exact`

Expected: FAIL because `ActiveSessionState` does not store an open stream or refresh method.

- [ ] **Step 3: Store capture-stream descriptors and live frame refresh**

```rust
pub struct ActiveSessionState {
    pub summary: DeviceSessionSummary,
    pub selected_source_id: Option<String>,
    pub capture_sources: Vec<VideoSource>,
    pub capture_stream: Option<CaptureStreamDescriptor>,
    pub latest_frame: Option<VideoFrameDescriptor>,
    pub control_checklist: ControlSetupChecklist,
    pub diagnostics: SessionDiagnostics,
    pub execution_result: Option<ExecutionResult>,
    capture_plugin: Option<RunningPlugin>,
    control_plugin: Option<RunningPlugin>,
    grounding_plugin: Option<RunningPlugin>,
}

impl ActiveSessionState {
    pub async fn refresh_capture_frame(&mut self) -> Result<VideoFrameDescriptor> {
        let capture = self.capture_plugin.as_mut().ok_or_else(|| anyhow!("missing capture plugin"))?;
        let source_id = self
            .selected_source_id
            .clone()
            .ok_or_else(|| anyhow!("missing selected source id"))?;

        let frame = match request_plugin(capture, &HostToPlugin::ReadCaptureFrame).await? {
            PluginToHost::CaptureFrame { frame } => frame,
            other => return Err(anyhow!("unexpected capture refresh response: {other:?}")),
        };
        self.latest_frame = Some(frame.clone());
        Ok(frame)
    }
}
```

- [ ] **Step 4: Run the runtime and orchestrator stream tests to verify they pass**

Run: `cargo test -p ios-control-session-orchestrator --test mock_flow`

Expected: PASS

Run: `cargo test -p ios-control-plugin-runtime --test plugin_roundtrip`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/session-orchestrator/src/lib.rs \
  crates/plugin-runtime/tests/plugin_roundtrip.rs \
  crates/session-orchestrator/tests/mock_flow.rs \
  crates/session-orchestrator/tests/local_mock_e2e.rs \
  plugins/capture-window/src/main.rs \
  plugins/capture-direct/src/main.rs
git commit -m "feat: keep live capture streams open for session refresh"
```

### Task 3: Render The Preview In The Host App

**Files:**
- Create: `apps/host-desktop/src/preview.rs`
- Modify: `apps/host-desktop/src/lib.rs`
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/src/panels/session_view.rs`
- Modify: `apps/host-desktop/tests/support/mod.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`
- Create: `apps/host-desktop/tests/preview.rs`
- Test: `apps/host-desktop/tests/preview.rs`

- [ ] **Step 1: Write the failing host-preview tests**

```rust
use host_desktop::preview::color_image_from_slot;
use ios_control_contracts::capture::{CaptureStreamDescriptor, SourceKind};

mod support;

#[test]
fn color_image_from_slot_reads_rgba_frame() {
    let descriptor = CaptureStreamDescriptor {
        source_id: "window-helper-1".into(),
        source_kind: SourceKind::Window,
        width: 2,
        height: 1,
        rotation_degrees: 0,
        slot_bytes: 8,
        slot_path: support::write_slot_bytes(&[255, 0, 0, 255, 0, 255, 0, 255]),
    };

    let image = color_image_from_slot(&descriptor).unwrap();
    assert_eq!(image.size, [2, 1]);
    assert_eq!(image.pixels.len(), 2);
}
```

- [ ] **Step 2: Run the preview test to verify it fails**

Run: `cargo test -p host-desktop color_image_from_slot_reads_rgba_frame -- --exact`

Expected: FAIL because `preview.rs` and `color_image_from_slot` do not exist yet.

- [ ] **Step 3: Add preview loading and session rendering**

```rust
pub fn color_image_from_slot(stream: &CaptureStreamDescriptor) -> anyhow::Result<egui::ColorImage> {
    let reader = FrameSlotReader::open(Path::new(&stream.slot_path), stream.slot_bytes as usize)?;
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [stream.width as usize, stream.height as usize],
        reader.read(),
    ))
}

pub fn render(ui: &mut Ui, view_model: &SessionViewModel, texture: Option<&egui::TextureHandle>) -> SessionAction {
    if let Some(texture) = texture {
        ui.image(texture);
    } else if let Some(frame) = &view_model.latest_frame {
        ui.label(format!("{}x{} frame {}", frame.width, frame.height, frame.frame_index));
    }
    SessionAction::None
}

pub fn write_slot_bytes(bytes: &[u8]) -> String {
    let mut slot = FrameSlot::new(bytes.len()).unwrap();
    slot.write(bytes).unwrap();
    slot.path().display().to_string()
}
```

- [ ] **Step 4: Run the host-desktop test suite to verify preview rendering support passes**

Run: `cargo test -p host-desktop`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/host-desktop/src/preview.rs \
  apps/host-desktop/src/lib.rs \
  apps/host-desktop/src/app.rs \
  apps/host-desktop/src/panels/session_view.rs \
  apps/host-desktop/tests/support/mod.rs \
  apps/host-desktop/tests/app_state.rs \
  apps/host-desktop/tests/preview.rs
git commit -m "feat: render live preview frames in host app"
```
