# Host Desktop Global Direct Receiver Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a global `Start Direct Receiver` workflow to `host-desktop` that launches a direct capture session from inside the app and preserves the existing window-capture flow.

**Architecture:** Resolve both window and direct capture plugins during host bootstrap, make capture backend choice explicit in the runtime/orchestrator session start path, and surface direct-receiver readiness plus a global start action in the host app. The direct plugin will self-host its helper mode by receiving `IOS_CONTROL_DIRECT_RECEIVER_HELPER` pointed at its own executable path.

**Tech Stack:** Rust, egui/eframe, Tokio, existing plugin protocol/runtime, existing host-desktop integration tests

---

### Task 1: Add Failing Coverage For Direct Capture Resolution And Selection

**Files:**
- Modify: `apps/host-desktop/tests/bootstrap_locator.rs`
- Modify: `crates/session-orchestrator/tests/mock_flow.rs`
- Test: `apps/host-desktop/tests/bootstrap_locator.rs`
- Test: `crates/session-orchestrator/tests/mock_flow.rs`

- [ ] **Step 1: Write the failing bootstrap locator test for the direct plugin path**

```rust
#[test]
fn locator_resolves_direct_capture_plugin_for_bundle_and_workspace_layouts() {
    let staged = stage_bundle_layout();

    let bundle = locate_runtime_layout(RuntimeLocatorInput {
        executable_path: staged.host_exe.clone(),
        manifest_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        cargo_target_dir: None,
        cargo_build_target: None,
    })
    .expect("bundle layout should resolve");

    assert_eq!(
        bundle.plugin_paths.capture_direct,
        staged
            .plugins_dir
            .join(format!("plugin-capture-direct{}", std::env::consts::EXE_SUFFIX))
    );

    let root = workspace_root();
    let exe = target_dir(&root).join(format!("debug/host-desktop{}", std::env::consts::EXE_SUFFIX));
    let workspace = locate_runtime_layout(RuntimeLocatorInput {
        executable_path: exe,
        manifest_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        cargo_target_dir: std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from),
        cargo_build_target: std::env::var_os("CARGO_BUILD_TARGET").map(PathBuf::from),
    })
    .expect("workspace layout should resolve");

    assert_eq!(
        workspace.plugin_paths.capture_direct,
        target_dir(&root).join(format!(
            "debug/plugin-capture-direct{}",
            std::env::consts::EXE_SUFFIX
        ))
    );
}
```

- [ ] **Step 2: Run the locator test to verify it fails**

Run: `cargo test -p host-desktop --test bootstrap_locator locator_resolves_direct_capture_plugin_for_bundle_and_workspace_layouts -- --exact`

Expected: FAIL because `PluginPaths` and the runtime locator do not yet expose `capture_direct`.

- [ ] **Step 3: Write the failing orchestrator test for explicit direct capture startup**

```rust
#[tokio::test]
async fn start_session_with_direct_backend_uses_capture_direct_plugin() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);

    let mut orchestrator = SessionOrchestrator::default();
    let state = orchestrator
        .start_session_with_plugins(StartSessionRequest {
            device_id: "direct-session".into(),
            device_name: "Direct Receiver".into(),
            selected_source_id: Some("direct-1".into()),
            capture_backend: CaptureBackend::Direct,
            plugin_paths: PluginPaths {
                capture: plugin_path(&root, "plugin-capture-window"),
                capture_direct: plugin_path(&root, "plugin-capture-direct"),
                control_ble: plugin_path(&root, "plugin-control-ble"),
                control_fallback: plugin_path(&root, "plugin-control-window-bridge"),
                grounding: Some(plugin_path(&root, "plugin-grounding-core")),
            },
        })
        .await
        .unwrap();

    assert_eq!(state.summary.capture_plugin.as_deref(), Some("capture.direct"));
    assert_eq!(state.selected_source_id.as_deref(), Some("direct-1"));
    assert_eq!(state.latest_frame.as_ref().unwrap().source_id, "direct-1");

    state.shutdown().await.unwrap();
}
```

- [ ] **Step 4: Run the orchestrator test to verify it fails**

Run: `cargo test -p ios-control-session-orchestrator --test mock_flow start_session_with_direct_backend_uses_capture_direct_plugin -- --exact`

Expected: FAIL because `StartSessionRequest` does not yet accept a capture backend and `PluginPaths` does not yet carry a direct capture path.

### Task 2: Implement Explicit Direct Capture Backend Selection

**Files:**
- Modify: `crates/session-orchestrator/src/lib.rs`
- Modify: `crates/session-orchestrator/tests/mock_flow.rs`
- Modify: `apps/host-desktop/src/bootstrap/runtime_locator.rs`
- Modify: `apps/host-desktop/src/runtime.rs`
- Modify: `apps/host-desktop/tests/bootstrap_locator.rs`
- Test: `apps/host-desktop/tests/bootstrap_locator.rs`
- Test: `crates/session-orchestrator/tests/mock_flow.rs`

- [ ] **Step 1: Add the new shared capture-backend and direct-path fields**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackend {
    Window,
    Direct,
}

#[derive(Debug, Clone)]
pub struct PluginPaths {
    pub capture: PathBuf,
    pub capture_direct: PathBuf,
    pub control_ble: PathBuf,
    pub control_fallback: PathBuf,
    pub grounding: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct StartSessionRequest {
    pub device_id: String,
    pub device_name: String,
    pub selected_source_id: Option<String>,
    pub capture_backend: CaptureBackend,
    pub plugin_paths: PluginPaths,
}
```

- [ ] **Step 2: Route session startup through the selected capture backend**

```rust
let capture_path = match request.capture_backend {
    CaptureBackend::Window => request.plugin_paths.capture.clone(),
    CaptureBackend::Direct => request.plugin_paths.capture_direct.clone(),
};

let mut capture = RunningPlugin::spawn(&capture_path).await?;
```

For the direct path, set `IOS_CONTROL_DIRECT_RECEIVER_HELPER` in the child process environment before spawning the plugin. Keep the current window-capture behavior unchanged.

- [ ] **Step 3: Resolve `plugin-capture-direct` from host bootstrap**

```rust
PluginPaths {
    capture: dir.join(format!("plugin-capture-window{}", std::env::consts::EXE_SUFFIX)),
    capture_direct: dir.join(format!("plugin-capture-direct{}", std::env::consts::EXE_SUFFIX)),
    control_ble: dir.join(format!("plugin-control-ble{}", std::env::consts::EXE_SUFFIX)),
    control_fallback: dir.join(format!(
        "plugin-control-window-bridge{}",
        std::env::consts::EXE_SUFFIX
    )),
    grounding: Some(dir.join(format!(
        "plugin-grounding-core{}",
        std::env::consts::EXE_SUFFIX
    ))),
}
```

- [ ] **Step 4: Update the host runtime start API to accept a capture backend**

```rust
pub fn start_session(
    &mut self,
    device_id: &str,
    device_name: &str,
    selected_source_id: Option<String>,
    capture_backend: CaptureBackend,
) -> Result<HostRuntimeSnapshot> {
    self.tokio.block_on(
        self.supervisor.start_or_replace_session(StartSessionRequest {
            device_id: device_id.into(),
            device_name: device_name.into(),
            selected_source_id,
            capture_backend,
            plugin_paths: self.config.plugin_paths.clone(),
        }),
    )?;

    self.snapshot(device_id)
}
```

- [ ] **Step 5: Run the focused tests to verify they pass**

Run:
- `cargo test -p host-desktop --test bootstrap_locator locator_resolves_direct_capture_plugin_for_bundle_and_workspace_layouts -- --exact`
- `cargo test -p ios-control-session-orchestrator --test mock_flow start_session_with_direct_backend_uses_capture_direct_plugin -- --exact`

Expected: PASS

### Task 3: Add Failing Host-App Coverage For Global Direct Receiver Controls

**Files:**
- Modify: `apps/host-desktop/tests/app_state.rs`
- Modify: `apps/host-desktop/tests/support/mod.rs`
- Test: `apps/host-desktop/tests/app_state.rs`

- [ ] **Step 1: Write the failing host-app test for startup readiness exposing a direct receiver action**

```rust
#[test]
fn host_app_enables_global_direct_receiver_when_direct_backend_is_ready() {
    let mut fixture = host_app_with_runtime();

    assert!(fixture.app.startup.direct_receiver_available);
    assert!(fixture.app.can_start_direct_receiver());
}
```

- [ ] **Step 2: Run the host-app readiness test to verify it fails**

Run: `cargo test -p host-desktop --test app_state host_app_enables_global_direct_receiver_when_direct_backend_is_ready -- --exact`

Expected: FAIL because `StartupViewModel` and `HostDesktopApp` do not yet expose direct-receiver readiness or a global action.

- [ ] **Step 3: Write the failing host-app test for starting a direct receiver session**

```rust
#[test]
fn host_app_start_direct_receiver_uses_direct_capture_source() {
    let mut fixture = host_app_with_runtime();

    fixture.app.request_start_direct_receiver();

    assert!(matches!(
        fixture.app.session.ui_state,
        SessionUiState::Streaming | SessionUiState::Starting
    ));
    assert_eq!(
        fixture.app.session.selected_source.as_ref().map(|source| source.source_id.as_str()),
        Some("direct-1")
    );
}
```

- [ ] **Step 4: Run the direct-start host-app test to verify it fails**

Run: `cargo test -p host-desktop --test app_state host_app_start_direct_receiver_uses_direct_capture_source -- --exact`

Expected: FAIL because the host app does not yet have a direct-receiver action path.

### Task 4: Implement Host-Level Direct Receiver UI And State

**Files:**
- Modify: `apps/host-desktop/src/view_models/startup.rs`
- Modify: `apps/host-desktop/src/bootstrap/capability_probe.rs`
- Modify: `apps/host-desktop/src/panels/startup.rs`
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/src/main.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`
- Modify: `apps/host-desktop/tests/support/mod.rs`
- Test: `apps/host-desktop/tests/app_state.rs`

- [ ] **Step 1: Extend startup readiness with direct-receiver state**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectReceiverState {
    pub available: bool,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupViewModel {
    pub readiness: StartupReadiness,
    pub summary: String,
    pub items: Vec<StartupItem>,
    pub direct_receiver: DirectReceiverState,
}
```

Populate `direct_receiver` from the direct capture probe while preserving the existing startup summary.

- [ ] **Step 2: Render the global direct receiver action in the startup panel**

```rust
pub enum StartupAction {
    None,
    StartDirectReceiver,
}

pub fn render(ui: &mut Ui, view_model: &StartupViewModel) -> StartupAction {
    let mut action = StartupAction::None;

    ui.heading("Startup Readiness");
    ui.label(&view_model.summary);
    ui.label(format!(
        "Direct Receiver | {} | {}",
        view_model.direct_receiver.status,
        view_model.direct_receiver.detail
    ));
    if ui
        .add_enabled(
            view_model.direct_receiver.available,
            egui::Button::new("Start Direct Receiver"),
        )
        .clicked()
    {
        action = StartupAction::StartDirectReceiver;
    }

    for item in &view_model.items {
        ui.label(format!("{} | {} | {}", item.label, item.status, item.detail));
    }

    action
}
```

- [ ] **Step 3: Add the host-app direct-receiver action path**

```rust
pub fn can_start_direct_receiver(&self) -> bool {
    self.startup.direct_receiver.available
        && !matches!(self.session.ui_state, SessionUiState::Starting | SessionUiState::Streaming)
}

pub fn request_start_direct_receiver(&mut self) {
    self.selected_device_id = Some("direct-receiver".into());
    self.session = SessionViewModel::starting();
    self.start_runtime_session("direct-receiver", "Direct Receiver", Some("direct-1".into()), CaptureBackend::Direct);
}
```

Preserve the existing device-selection flow for normal window-capture sessions.

- [ ] **Step 4: Handle the startup panel action during app update**

```rust
match startup::render(ui, &self.startup) {
    StartupAction::StartDirectReceiver => self.request_start_direct_receiver(),
    StartupAction::None => {}
}
```

- [ ] **Step 5: Run the focused host tests to verify they pass**

Run:
- `cargo test -p host-desktop --test app_state host_app_enables_global_direct_receiver_when_direct_backend_is_ready -- --exact`
- `cargo test -p host-desktop --test app_state host_app_start_direct_receiver_uses_direct_capture_source -- --exact`

Expected: PASS

### Task 5: Regression Verification

**Files:**
- Modify: `apps/host-desktop/tests/support/mod.rs`
- Modify: `crates/session-orchestrator/tests/support/mod.rs`
- Test: `apps/host-desktop/tests/app_state.rs`
- Test: `apps/host-desktop/tests/bootstrap_locator.rs`
- Test: `crates/session-orchestrator/tests/mock_flow.rs`

- [ ] **Step 1: Make sure test support builds the direct plugin and prepares its env as needed**

```rust
pub fn host_plugin_paths(workspace_root: &Path) -> PluginPaths {
    PluginPaths {
        capture: plugin_path(workspace_root, "plugin-capture-window"),
        capture_direct: plugin_path(workspace_root, "plugin-capture-direct"),
        control_ble: plugin_path(workspace_root, "plugin-control-ble"),
        control_fallback: plugin_path(workspace_root, "plugin-control-window-bridge"),
        grounding: Some(plugin_path(workspace_root, "plugin-grounding-core")),
    }
}
```

- [ ] **Step 2: Run the focused host and orchestrator suites**

Run:
- `cargo test -p host-desktop --test bootstrap_locator`
- `cargo test -p host-desktop --test app_state`
- `cargo test -p ios-control-session-orchestrator --test mock_flow`

Expected: PASS

- [ ] **Step 3: Run the package-level regression checks for the touched crates**

Run:
- `cargo test -p host-desktop`
- `cargo test -p ios-control-session-orchestrator`

Expected: PASS
