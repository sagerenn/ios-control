# Control Execution And Observation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn control from probe-only helper status into a structured prepare/execute path that reports observable action state back to the runtime and UI.

**Architecture:** Keep helper-backed control as the near-term production path, but make the helper contract explicit and structured for both BLE and mirrored-window fallback. Thread control phase, execution result, and observed-change signals through the orchestrator and host runtime so the desktop UI can present real setup, execution, and recovery state instead of generic summaries.

**Tech Stack:** Rust, serde JSON helper contracts, `ios-control-contracts`, `ios-control-session-orchestrator`, helper-backed BLE and window-input plugins

---

## File Structure

- `crates/contracts/src/control.rs`: extend execution payloads with observed-change data.
- `crates/contracts/tests/control_contract.rs`: regressions for the updated control contract.
- `plugins/control-ble/src/helper_bridge.rs`: parse structured prepare and execute helper replies.
- `plugins/control-ble/src/main.rs`: use helper replies to build real control session and execution summaries.
- `plugins/control-ble/tests/helper_contract.rs`: helper-backed BLE prepare/execute coverage.
- `plugins/control-window-bridge/src/helper_launcher.rs`: return structured execution output instead of exit-status-only success.
- `plugins/control-window-bridge/src/main.rs`: report detailed fallback execution results.
- `plugins/control-window-bridge/tests/contract.rs`: structured fallback-helper coverage.
- `crates/session-orchestrator/src/lib.rs`: store control phase and observed-change state in diagnostics.
- `apps/host-desktop/src/runtime.rs`: include control phase in runtime snapshots.
- `apps/host-desktop/src/app.rs`: display setup, paired, failure, and reconnect information.
- `apps/host-desktop/tests/app_state.rs`: host diagnostics regression coverage.

### Task 1: Extend The BLE Control Helper Contract

**Files:**
- Modify: `crates/contracts/src/control.rs`
- Create: `crates/contracts/tests/control_contract.rs`
- Modify: `plugins/control-ble/src/helper_bridge.rs`
- Modify: `plugins/control-ble/src/main.rs`
- Create: `plugins/control-ble/tests/helper_contract.rs`
- Test: `plugins/control-ble/tests/helper_contract.rs`

- [ ] **Step 1: Write the failing BLE helper-contract tests**

```rust
#[test]
fn ble_helper_prepare_returns_control_phase_and_checklist() {
    let helper = support::write_ble_helper(
        r#"{"supported":true,"supports_prepare":true,"supports_execute":true}"#,
        r#"{"phase":"Advertising","checklist":["Enable Bluetooth","Pair the device"],"notes":["Waiting for iPhone"]}"#,
        r#"{"phase":"Succeeded","summary":"pointer action applied","observed_change":true}"#,
    );

    let prepare = run_prepare(&helper).unwrap();
    assert_eq!(prepare.phase, "Advertising");
    assert_eq!(prepare.checklist.len(), 2);
}

#[test]
fn ble_helper_execute_exposes_observed_change() {
    let helper = support::write_ble_helper(
        r#"{"supported":true,"supports_prepare":true,"supports_execute":true}"#,
        r#"{"phase":"Connected","checklist":[],"notes":[]}"#,
        r#"{"phase":"Succeeded","summary":"tap applied","observed_change":true}"#,
    );

    let execution = run_execute(&helper, "pointer").unwrap();
    assert!(execution.observed_change);
}
```

- [ ] **Step 2: Run the BLE tests to verify they fail**

Run: `cargo test -p plugin-control-ble --test helper_contract`

Expected: FAIL because `run_prepare()` returns `Result<()>` and `BleHelperExecution` does not include `observed_change`.

- [ ] **Step 3: Implement the structured BLE helper contract**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BleHelperPrepare {
    pub phase: String,
    pub checklist: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BleHelperExecution {
    pub phase: String,
    pub summary: String,
    #[serde(default)]
    pub observed_change: bool,
    #[serde(default)]
    pub failure_reason: Option<String>,
}

pub fn run_prepare(helper: &Path) -> Result<BleHelperPrepare> {
    let mut command = Command::new(helper);
    command.arg("prepare");
    let output = run_for_output(command, "ble helper prepare")?;
    if !output.status.success() {
        return Err(anyhow!("ble helper prepare failed"));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub summary: String,
    pub phase: ExecutionPhase,
    pub observed_change: Option<bool>,
    pub failure_reason: Option<String>,
}
```

- [ ] **Step 4: Run the BLE tests and the control-contract tests to verify they pass**

Run: `cargo test -p plugin-control-ble`

Expected: PASS

Run: `cargo test -p ios-control-contracts --test control_contract`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/contracts/src/control.rs \
  crates/contracts/tests/control_contract.rs \
  plugins/control-ble/src/helper_bridge.rs \
  plugins/control-ble/src/main.rs \
  plugins/control-ble/tests/helper_contract.rs
git commit -m "feat: add structured ble control helper contract"
```

### Task 2: Make Window Fallback Execution Structured

**Files:**
- Modify: `plugins/control-window-bridge/src/helper_launcher.rs`
- Modify: `plugins/control-window-bridge/src/main.rs`
- Modify: `plugins/control-window-bridge/tests/contract.rs`
- Test: `plugins/control-window-bridge/tests/contract.rs`

- [ ] **Step 1: Write the failing fallback-helper tests**

```rust
#[test]
fn window_bridge_helper_returns_structured_execution_summary() {
    let helper = support::write_window_helper(
        r#"{"phase":"Succeeded","summary":"window click applied","observed_change":true}"#,
    );

    let execution = launch_helper_json(helper, &["--source".into(), "window-helper-1".into(), "--pointer-plan".into()]).unwrap();
    assert_eq!(execution.summary, "window click applied");
    assert!(execution.observed_change);
}
```

- [ ] **Step 2: Run the contract test to verify it fails**

Run: `cargo test -p plugin-control-window-bridge --test contract`

Expected: FAIL because the helper launcher only returns `ExitStatus`.

- [ ] **Step 3: Return structured JSON from the fallback helper and plugin**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WindowHelperExecution {
    pub phase: String,
    pub summary: String,
    #[serde(default)]
    pub observed_change: bool,
    #[serde(default)]
    pub failure_reason: Option<String>,
}

pub fn launch_helper_json(helper: PathBuf, args: &[String]) -> anyhow::Result<WindowHelperExecution> {
    let output = Command::new(helper)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    if !output.status.success() {
        return Err(anyhow!("window helper returned non-zero exit status"));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

let helper_summary = launch_helper_json(helper, &command.args)?;
let summary = ExecutionSummary {
    summary: helper_summary.summary,
    phase: map_execution_phase(&helper_summary.phase),
    observed_change: Some(helper_summary.observed_change),
    failure_reason: helper_summary.failure_reason,
};
```

- [ ] **Step 4: Run the fallback helper tests to verify they pass**

Run: `cargo test -p plugin-control-window-bridge --test contract`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/control-window-bridge/src/helper_launcher.rs \
  plugins/control-window-bridge/src/main.rs \
  plugins/control-window-bridge/tests/contract.rs
git commit -m "feat: add structured fallback control execution results"
```

### Task 3: Thread Control Phase And Observation Into Runtime And UI

**Files:**
- Modify: `crates/session-orchestrator/src/lib.rs`
- Modify: `apps/host-desktop/src/runtime.rs`
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`
- Test: `apps/host-desktop/tests/app_state.rs`

- [ ] **Step 1: Write the failing runtime/UI tests**

```rust
#[test]
fn runtime_snapshot_preserves_control_phase_and_observed_change() {
    let snapshot = support::runtime_snapshot_with_control(
        ControlSessionPhase::Advertising,
        "Waiting for iPhone",
        Some(true),
    );

    assert_eq!(snapshot.workspace.control_phase, ControlSessionPhase::Advertising);
    assert_eq!(snapshot.workspace.execution_observed_change, Some(true));
}

#[test]
fn host_app_surfaces_reconnect_guidance_for_degraded_control() {
    let mut app = support::host_app_from_runtime_snapshot(support::runtime_snapshot_with_control(
        ControlSessionPhase::Error,
        "Reconnect BLE helper",
        Some(false),
    ));

    assert!(app.diagnostics.control_summary.contains("Reconnect BLE helper"));
}
```

- [ ] **Step 2: Run the app-state tests to verify they fail**

Run: `cargo test -p host-desktop host_app_surfaces_reconnect_guidance_for_degraded_control -- --exact`

Expected: FAIL because runtime snapshots do not carry structured control-phase data yet.

- [ ] **Step 3: Extend runtime snapshots and host diagnostics**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWorkspaceState {
    pub device_id: String,
    pub summary: DeviceSessionSummary,
    pub capture_sources: Vec<VideoSource>,
    pub selected_source_id: Option<String>,
    pub control_checklist: ControlSetupChecklist,
    pub control_phase: ControlSessionPhase,
    pub execution_observed_change: Option<bool>,
    pub diagnostics: SessionDiagnostics,
}

pub struct SessionDiagnostics {
    pub control_phase: ControlSessionPhase,
    pub control_summary: String,
    pub grounding_summary: Option<String>,
}

self.diagnostics.control_summary = format!(
    "{:?}: {}",
    snapshot.workspace.control_phase,
    snapshot.workspace.diagnostics.control_summary
);
```

- [ ] **Step 4: Run the host and orchestrator test suites to verify structured control state passes**

Run: `cargo test -p host-desktop`

Expected: PASS

Run: `cargo test -p ios-control-session-orchestrator`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/session-orchestrator/src/lib.rs \
  apps/host-desktop/src/runtime.rs \
  apps/host-desktop/src/app.rs \
  apps/host-desktop/tests/app_state.rs
git commit -m "feat: surface structured control execution state in host runtime"
```
