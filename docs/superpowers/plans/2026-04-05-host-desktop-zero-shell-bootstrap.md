# Host Desktop Zero-Shell Bootstrap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `host-desktop` start without required shell setup by resolving its own runtime components, surfacing honest startup readiness, and supporting both repo launches and packaged bundle launches.

**Architecture:** Add a host-owned bootstrap layer that resolves plugin/helper paths from either the current executable layout or the repo target layout, probes available backends before session start, and feeds the app an explicit startup readiness snapshot. Keep the existing plugin-based runtime, but stop using fake default device state as the startup UI.

**Tech Stack:** Rust, eframe/egui, Tokio, existing plugin protocol/runtime crates, Cargo tests

---

## File Structure

- Create: `apps/host-desktop/src/bootstrap/mod.rs`
  - bootstrap entrypoints and module exports
- Create: `apps/host-desktop/src/bootstrap/model.rs`
  - startup path records, probe results, and readiness enums
- Create: `apps/host-desktop/src/bootstrap/runtime_locator.rs`
  - repo-mode and bundle-mode path resolution
- Create: `apps/host-desktop/src/bootstrap/capability_probe.rs`
  - plugin handshake/probe helpers and startup snapshot construction
- Create: `apps/host-desktop/src/panels/startup.rs`
  - render the startup readiness summary and next steps
- Create: `apps/host-desktop/src/view_models/startup.rs`
  - startup panel view models
- Modify: `apps/host-desktop/src/lib.rs`
  - export bootstrap and startup modules
- Modify: `apps/host-desktop/src/main.rs`
  - replace direct plugin path wiring with bootstrap-driven runtime config
- Modify: `apps/host-desktop/src/app.rs`
  - remove fake startup device state, store startup readiness, render startup panel
- Modify: `apps/host-desktop/src/runtime.rs`
  - accept bootstrap-resolved paths cleanly and expose bootstrap snapshot if needed
- Modify: `apps/host-desktop/src/panels/session_view.rs`
  - ensure blocked startup still renders a coherent session area
- Modify: `apps/host-desktop/src/panels/settings.rs`
  - optionally list resolved plugin paths or readiness rows
- Modify: `apps/host-desktop/src/view_models/diagnostics.rs`
  - carry startup diagnostics separately from runtime errors
- Modify: `apps/host-desktop/tests/support/mod.rs`
  - add staged-bundle helpers and path-fixture helpers
- Modify: `apps/host-desktop/tests/app_state.rs`
  - cover blocked/partial/ready startup behavior
- Modify: `apps/host-desktop/tests/runtime_integration.rs`
  - cover repo-mode startup without env vars
- Create: `apps/host-desktop/tests/bootstrap_locator.rs`
  - focused locator and packaged-bundle tests
- Modify: `scripts/package_release.py`
  - keep the runtime bundle layout aligned with the locator contract if needed

### Task 1: Add Runtime Locator And Bootstrap Models

**Files:**
- Create: `apps/host-desktop/src/bootstrap/mod.rs`
- Create: `apps/host-desktop/src/bootstrap/model.rs`
- Create: `apps/host-desktop/src/bootstrap/runtime_locator.rs`
- Modify: `apps/host-desktop/src/lib.rs`
- Modify: `apps/host-desktop/tests/support/mod.rs`
- Create: `apps/host-desktop/tests/bootstrap_locator.rs`
- Test: `apps/host-desktop/tests/bootstrap_locator.rs`

- [ ] **Step 1: Write the failing bootstrap locator tests**

```rust
use std::path::PathBuf;

use host_desktop::bootstrap::runtime_locator::{
    locate_runtime_layout, RuntimeLayoutKind, RuntimeLocatorInput,
};

mod support;
use support::{stage_bundle_layout, workspace_root};

#[test]
fn locator_prefers_bundle_layout_from_executable_path() {
    let staged = stage_bundle_layout();

    let layout = locate_runtime_layout(RuntimeLocatorInput {
        executable_path: staged.host_exe.clone(),
        manifest_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        cargo_target_dir: None,
        cargo_build_target: None,
        env_overrides: Default::default(),
    })
    .unwrap();

    assert_eq!(layout.kind, RuntimeLayoutKind::Bundle);
    assert_eq!(layout.plugin_paths.capture, staged.plugins_dir.join(format!("plugin-capture-window{}", std::env::consts::EXE_SUFFIX)));
    assert_eq!(layout.plugin_paths.control_ble, staged.plugins_dir.join(format!("plugin-control-ble{}", std::env::consts::EXE_SUFFIX)));
}

#[test]
fn locator_falls_back_to_workspace_target_layout_for_repo_launches() {
    let root = workspace_root();
    let exe = support::target_dir(&root).join(format!("debug/host-desktop{}", std::env::consts::EXE_SUFFIX));

    let layout = locate_runtime_layout(RuntimeLocatorInput {
        executable_path: exe,
        manifest_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        cargo_target_dir: std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from),
        cargo_build_target: std::env::var_os("CARGO_BUILD_TARGET").map(PathBuf::from),
        env_overrides: Default::default(),
    })
    .unwrap();

    assert_eq!(layout.kind, RuntimeLayoutKind::Workspace);
    assert!(layout.plugin_paths.capture.ends_with(format!("plugin-capture-window{}", std::env::consts::EXE_SUFFIX)));
}
```

- [ ] **Step 2: Run the locator tests to verify they fail**

Run: `cargo test -p host-desktop --test bootstrap_locator -- --nocapture`
Expected: FAIL with unresolved `bootstrap` imports and missing locator types/functions.

- [ ] **Step 3: Write the minimal bootstrap model and locator implementation**

```rust
// apps/host-desktop/src/bootstrap/model.rs
use std::collections::BTreeMap;
use std::path::PathBuf;

use ios_control_session_orchestrator::PluginPaths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeLayoutKind {
    Workspace,
    Bundle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeEnvOverrides {
    pub capture_helper: Option<PathBuf>,
    pub window_input_helper: Option<PathBuf>,
    pub ble_helper: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLayout {
    pub kind: RuntimeLayoutKind,
    pub root: PathBuf,
    pub plugin_paths: PluginPaths,
    pub helper_paths: BTreeMap<String, PathBuf>,
}
```

```rust
// apps/host-desktop/src/bootstrap/runtime_locator.rs
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use ios_control_session_orchestrator::PluginPaths;

use super::model::{RuntimeEnvOverrides, RuntimeLayout, RuntimeLayoutKind};

#[derive(Debug, Clone)]
pub struct RuntimeLocatorInput {
    pub executable_path: PathBuf,
    pub manifest_dir: PathBuf,
    pub cargo_target_dir: Option<PathBuf>,
    pub cargo_build_target: Option<PathBuf>,
    pub env_overrides: RuntimeEnvOverrides,
}

pub fn locate_runtime_layout(input: RuntimeLocatorInput) -> Result<RuntimeLayout> {
    let exe_dir = input
        .executable_path
        .parent()
        .ok_or_else(|| anyhow!("executable path has no parent"))?
        .to_path_buf();

    if exe_dir.ends_with("bin") && exe_dir.parent().is_some() {
        let root = exe_dir.parent().unwrap().to_path_buf();
        let plugins_dir = root.join("plugins");
        return Ok(RuntimeLayout {
            kind: RuntimeLayoutKind::Bundle,
            root: root.clone(),
            plugin_paths: bundle_plugin_paths(&plugins_dir),
            helper_paths: Default::default(),
        });
    }

    let workspace_root = input
        .manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| anyhow!("workspace root missing"))?
        .to_path_buf();
    let mut target_dir = input
        .cargo_target_dir
        .unwrap_or_else(|| workspace_root.join("target"));
    if target_dir.is_relative() {
        target_dir = workspace_root.join(target_dir);
    }
    if let Some(target) = input.cargo_build_target {
        target_dir.push(target);
    }

    Ok(RuntimeLayout {
        kind: RuntimeLayoutKind::Workspace,
        root: workspace_root,
        plugin_paths: workspace_plugin_paths(&target_dir),
        helper_paths: Default::default(),
    })
}

fn bundle_plugin_paths(plugins_dir: &Path) -> PluginPaths {
    PluginPaths {
        capture: plugins_dir.join(format!("plugin-capture-window{}", std::env::consts::EXE_SUFFIX)),
        control_ble: plugins_dir.join(format!("plugin-control-ble{}", std::env::consts::EXE_SUFFIX)),
        control_fallback: plugins_dir.join(format!("plugin-control-window-bridge{}", std::env::consts::EXE_SUFFIX)),
        grounding: Some(plugins_dir.join(format!("plugin-grounding-core{}", std::env::consts::EXE_SUFFIX))),
    }
}

fn workspace_plugin_paths(target_dir: &Path) -> PluginPaths {
    let debug_dir = target_dir.join("debug");
    PluginPaths {
        capture: debug_dir.join(format!("plugin-capture-window{}", std::env::consts::EXE_SUFFIX)),
        control_ble: debug_dir.join(format!("plugin-control-ble{}", std::env::consts::EXE_SUFFIX)),
        control_fallback: debug_dir.join(format!("plugin-control-window-bridge{}", std::env::consts::EXE_SUFFIX)),
        grounding: Some(debug_dir.join(format!("plugin-grounding-core{}", std::env::consts::EXE_SUFFIX))),
    }
}
```

- [ ] **Step 4: Add staged bundle test support**

```rust
// apps/host-desktop/tests/support/mod.rs
pub struct StagedBundleLayout {
    pub root: PathBuf,
    pub host_exe: PathBuf,
    pub plugins_dir: PathBuf,
}

pub fn stage_bundle_layout() -> StagedBundleLayout {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("host-desktop-bundle-{nonce}"));
    let bin_dir = root.join("bin");
    let plugins_dir = root.join("plugins");
    std::fs::create_dir_all(&bin_dir).unwrap();
    std::fs::create_dir_all(&plugins_dir).unwrap();

    let host_exe = bin_dir.join(format!("host-desktop{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&host_exe, b"stub").unwrap();

    for name in [
        "plugin-capture-window",
        "plugin-control-ble",
        "plugin-control-window-bridge",
        "plugin-grounding-core",
    ] {
        std::fs::write(
            plugins_dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
            b"stub",
        )
        .unwrap();
    }

    StagedBundleLayout {
        root,
        host_exe,
        plugins_dir,
    }
}
```

- [ ] **Step 5: Run the locator tests to verify they pass**

Run: `cargo test -p host-desktop --test bootstrap_locator -- --nocapture`
Expected: PASS with 2 tests passed.

- [ ] **Step 6: Commit**

```bash
git add apps/host-desktop/src/bootstrap/mod.rs \
  apps/host-desktop/src/bootstrap/model.rs \
  apps/host-desktop/src/bootstrap/runtime_locator.rs \
  apps/host-desktop/src/lib.rs \
  apps/host-desktop/tests/support/mod.rs \
  apps/host-desktop/tests/bootstrap_locator.rs
git commit -m "feat: add host runtime locator"
```

### Task 2: Add Startup Capability Snapshot And Honest Startup UI

**Files:**
- Create: `apps/host-desktop/src/bootstrap/capability_probe.rs`
- Create: `apps/host-desktop/src/panels/startup.rs`
- Create: `apps/host-desktop/src/view_models/startup.rs`
- Modify: `apps/host-desktop/src/lib.rs`
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/src/view_models/diagnostics.rs`
- Modify: `apps/host-desktop/src/panels/diagnostics.rs`
- Modify: `apps/host-desktop/src/panels/session_view.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`
- Test: `apps/host-desktop/tests/app_state.rs`

- [ ] **Step 1: Write the failing startup app-state tests**

```rust
#[test]
fn host_app_with_missing_runtime_components_starts_blocked_without_fake_device() {
    let fixture = host_app_with_missing_runtime_plugins_and_preferences("{}");

    assert_eq!(fixture.app.dashboard.total_devices, 0);
    assert!(fixture.app.available_device_ids.is_empty());
    assert!(fixture.app.selected_device_id.is_none());
    assert_eq!(fixture.app.session.ui_state, SessionUiState::Idle);
    assert!(fixture
        .app
        .diagnostics
        .startup_summary
        .contains("Blocked"));
    assert!(fixture
        .app
        .diagnostics
        .startup_items
        .iter()
        .any(|item| item.contains("plugin-capture-window")));
}

#[test]
fn host_app_new_no_longer_invents_mock_device_state() {
    let app = HostDesktopApp::new();

    assert_eq!(app.dashboard.total_devices, 0);
    assert!(app.available_device_ids.is_empty());
    assert_eq!(app.device_detail.device_name, "No device selected");
    assert!(app.device_detail.capture_sources.is_empty());
}
```

- [ ] **Step 2: Run the app-state tests to verify they fail**

Run: `cargo test -p host-desktop host_app_with_missing_runtime_components_starts_blocked_without_fake_device -- --exact`
Expected: FAIL because `DiagnosticsViewModel` has no startup fields and `HostDesktopApp::new()` still seeds fake device state.

- [ ] **Step 3: Write the minimal startup model and probe output types**

```rust
// apps/host-desktop/src/view_models/startup.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupReadiness {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupItem {
    pub label: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupViewModel {
    pub readiness: StartupReadiness,
    pub summary: String,
    pub items: Vec<StartupItem>,
}

impl StartupViewModel {
    pub fn blocked(items: Vec<StartupItem>) -> Self {
        Self {
            readiness: StartupReadiness::Blocked,
            summary: "Blocked: no usable device path yet".into(),
            items,
        }
    }
}
```

```rust
// apps/host-desktop/src/bootstrap/capability_probe.rs
use super::model::RuntimeLayout;
use crate::view_models::startup::{StartupItem, StartupReadiness, StartupViewModel};

pub fn startup_from_layout(layout: &RuntimeLayout) -> StartupViewModel {
    let mut items = Vec::new();

    items.push(item_for_path("Window Capture", &layout.plugin_paths.capture));
    items.push(item_for_path("BLE Control", &layout.plugin_paths.control_ble));
    items.push(item_for_path("Window Input Bridge", &layout.plugin_paths.control_fallback));
    if let Some(path) = layout.plugin_paths.grounding.as_ref() {
        items.push(item_for_path("Grounding", path));
    }

    let any_capture = layout.plugin_paths.capture.is_file();
    let any_control = layout.plugin_paths.control_ble.is_file() || layout.plugin_paths.control_fallback.is_file();
    let readiness = if any_capture && any_control {
        StartupReadiness::Ready
    } else if any_capture || any_control {
        StartupReadiness::Partial
    } else {
        StartupReadiness::Blocked
    };

    StartupViewModel {
        summary: match readiness {
            StartupReadiness::Ready => "Ready: runtime components resolved".into(),
            StartupReadiness::Partial => "Partial: some runtime components are unavailable".into(),
            StartupReadiness::Blocked => "Blocked: no usable device path yet".into(),
        },
        readiness,
        items,
    }
}

fn item_for_path(label: &str, path: &std::path::Path) -> StartupItem {
    if path.is_file() {
        StartupItem {
            label: label.into(),
            status: "Ready".into(),
            detail: path.display().to_string(),
        }
    } else {
        StartupItem {
            label: label.into(),
            status: "Missing".into(),
            detail: path.display().to_string(),
        }
    }
}
```

- [ ] **Step 4: Wire the startup view into app state and remove fake defaults**

```rust
// apps/host-desktop/src/view_models/diagnostics.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsViewModel {
    pub host_error: Option<String>,
    pub control_summary: String,
    pub grounding_summary: String,
    pub startup_summary: String,
    pub startup_items: Vec<String>,
}
```

```rust
// apps/host-desktop/src/app.rs (constructor excerpt)
dashboard: DashboardViewModel {
    total_devices: 0,
    degraded_devices: 0,
},
device_detail: DeviceDetailViewModel {
    device_name: "No device selected".into(),
    capture_sources: Vec::new(),
    active_source_id: None,
    control_checklist: ControlSetupChecklist { items: Vec::new() },
},
diagnostics: DiagnosticsViewModel {
    host_error: None,
    control_summary: "control not started".into(),
    grounding_summary: "grounding idle".into(),
    startup_summary: "Blocked: no usable device path yet".into(),
    startup_items: Vec::new(),
},
```

```rust
// apps/host-desktop/src/app.rs (new helper)
pub fn apply_startup_view(&mut self, startup: crate::view_models::startup::StartupViewModel) {
    self.diagnostics.startup_summary = startup.summary;
    self.diagnostics.startup_items = startup
        .items
        .into_iter()
        .map(|item| format!("{} | {} | {}", item.label, item.status, item.detail))
        .collect();
}
```

- [ ] **Step 5: Render startup diagnostics in the UI**

```rust
// apps/host-desktop/src/panels/startup.rs
use egui::Ui;
use crate::view_models::startup::StartupViewModel;

pub fn render(ui: &mut Ui, view_model: &StartupViewModel) {
    ui.heading("Startup Readiness");
    ui.label(&view_model.summary);
    for item in &view_model.items {
        ui.label(format!("{} | {} | {}", item.label, item.status, item.detail));
    }
}
```

```rust
// apps/host-desktop/src/panels/diagnostics.rs
pub fn render_startup(ui: &mut Ui, summary: &str, items: &[String]) {
    ui.heading("Startup Readiness");
    ui.label(summary);
    for item in items {
        ui.label(item);
    }
}
```

- [ ] **Step 6: Run the targeted app-state tests to verify they pass**

Run: `cargo test -p host-desktop host_app_new_no_longer_invents_mock_device_state -- --exact`
Expected: PASS

Run: `cargo test -p host-desktop host_app_with_missing_runtime_components_starts_blocked_without_fake_device -- --exact`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add apps/host-desktop/src/bootstrap/capability_probe.rs \
  apps/host-desktop/src/panels/startup.rs \
  apps/host-desktop/src/view_models/startup.rs \
  apps/host-desktop/src/lib.rs \
  apps/host-desktop/src/app.rs \
  apps/host-desktop/src/view_models/diagnostics.rs \
  apps/host-desktop/src/panels/diagnostics.rs \
  apps/host-desktop/src/panels/session_view.rs \
  apps/host-desktop/tests/app_state.rs
git commit -m "feat: add honest startup readiness state"
```

### Task 3: Wire Bootstrap Into Main Runtime Path And Verify Repo/Bundle Startup

**Files:**
- Modify: `apps/host-desktop/src/main.rs`
- Modify: `apps/host-desktop/src/runtime.rs`
- Modify: `apps/host-desktop/tests/runtime_integration.rs`
- Modify: `apps/host-desktop/tests/support/mod.rs`
- Modify: `scripts/package_release.py`
- Test: `apps/host-desktop/tests/runtime_integration.rs`
- Test: `apps/host-desktop/tests/bootstrap_locator.rs`

- [ ] **Step 1: Write the failing runtime/bootstrap integration tests**

```rust
#[test]
fn runtime_bootstrap_uses_repo_layout_without_env_vars() {
    let _lock = runtime_env_lock();
    let root = workspace_root();
    build_plugins(&root);

    let bootstrap = host_desktop::bootstrap::bootstrap_startup(
        support::target_dir(&root).join(format!("debug/host-desktop{}", std::env::consts::EXE_SUFFIX)),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();

    assert!(bootstrap.layout.plugin_paths.capture.ends_with(format!("plugin-capture-window{}", std::env::consts::EXE_SUFFIX)));
    assert!(!bootstrap.startup.summary.is_empty());
}
```

- [ ] **Step 2: Run the targeted runtime integration test to verify it fails**

Run: `cargo test -p host-desktop runtime_bootstrap_uses_repo_layout_without_env_vars -- --exact`
Expected: FAIL with missing `bootstrap_startup` or missing bootstrap exports.

- [ ] **Step 3: Add the bootstrap entrypoint and wire it into `main.rs`**

```rust
// apps/host-desktop/src/bootstrap/mod.rs
pub mod capability_probe;
pub mod model;
pub mod runtime_locator;

use std::path::PathBuf;

use anyhow::Result;

use capability_probe::startup_from_layout;
use model::RuntimeLayout;
use runtime_locator::{locate_runtime_layout, RuntimeLocatorInput};

#[derive(Debug, Clone)]
pub struct HostBootstrap {
    pub layout: RuntimeLayout,
    pub startup: crate::view_models::startup::StartupViewModel,
}

pub fn bootstrap_startup(executable_path: PathBuf, manifest_dir: PathBuf) -> Result<HostBootstrap> {
    let layout = locate_runtime_layout(RuntimeLocatorInput {
        executable_path,
        manifest_dir,
        cargo_target_dir: std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from),
        cargo_build_target: std::env::var_os("CARGO_BUILD_TARGET").map(PathBuf::from),
        env_overrides: Default::default(),
    })?;
    let startup = startup_from_layout(&layout);
    Ok(HostBootstrap { layout, startup })
}
```

```rust
// apps/host-desktop/src/main.rs (excerpt)
fn main() -> eframe::Result<()> {
    let executable_path = std::env::current_exe().expect("current exe path should resolve");
    let bootstrap = host_desktop::bootstrap::bootstrap_startup(
        executable_path,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    )
    .expect("startup bootstrap should resolve");
    let runtime_config = HostRuntimeConfig {
        plugin_paths: bootstrap.layout.plugin_paths.clone(),
    };

    eframe::run_native(
        "iOS Control Host",
        eframe::NativeOptions::default(),
        Box::new(move |_cc| {
            let mut app = host_desktop::app::HostDesktopApp::with_runtime(runtime_config.clone());
            app.apply_startup_view(bootstrap.startup.clone());
            Ok(Box::new(app))
        }),
    )
}
```

- [ ] **Step 4: Keep packaging aligned with the bundle locator**

```python
# scripts/package_release.py
_copy_binary(
    bin_dir=bin_dir,
    staged_path=bundle_root / "bin" / executable_name(HOST_BINARY, target),
    binary_name=HOST_BINARY,
    target=target,
)

for plugin in PLUGIN_BINARIES:
    _copy_binary(
        bin_dir=bin_dir,
        staged_path=bundle_root / "plugins" / executable_name(plugin, target),
        binary_name=plugin,
        target=target,
    )
```

If the existing script already satisfies the contract, keep behavior and only add regression coverage.

- [ ] **Step 5: Run the runtime/bootstrap tests to verify they pass**

Run: `cargo test -p host-desktop runtime_bootstrap_uses_repo_layout_without_env_vars -- --exact`
Expected: PASS

Run: `cargo test -p host-desktop --test runtime_integration -- --nocapture`
Expected: PASS

- [ ] **Step 6: Run the broader host test suite**

Run: `cargo test -p host-desktop`
Expected: PASS with all host-desktop tests green.

- [ ] **Step 7: Commit**

```bash
git add apps/host-desktop/src/bootstrap/mod.rs \
  apps/host-desktop/src/main.rs \
  apps/host-desktop/src/runtime.rs \
  apps/host-desktop/tests/runtime_integration.rs \
  apps/host-desktop/tests/support/mod.rs \
  scripts/package_release.py
git commit -m "feat: bootstrap host-desktop without shell setup"
```

## Self-Review

Spec coverage check:
- bootstrap ownership is covered by Task 1 and Task 3
- repo-mode and bundle-mode path resolution is covered by Task 1 and Task 3
- honest guided startup UX is covered by Task 2
- non-fatal startup failures are covered by Task 2
- packaging/layout alignment is covered by Task 3

Placeholder scan:
- all tasks include file paths, code, and explicit commands
- no `TODO` or `TBD` placeholders remain

Type consistency check:
- `RuntimeLayout`, `RuntimeLocatorInput`, `HostBootstrap`, `StartupViewModel`, and `StartupReadiness` are introduced before later tasks reference them
- the app-state changes refer consistently to `startup_summary` and `startup_items`

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-05-host-desktop-zero-shell-bootstrap.md`. Two execution options:

1. Subagent-Driven (recommended) - I dispatch a fresh subagent per task, review between tasks, fast iteration

2. Inline Execution - Execute tasks in this session using executing-plans, batch execution with checkpoints

Given the direct request to implement in this session, proceed with Inline Execution unless redirected.
