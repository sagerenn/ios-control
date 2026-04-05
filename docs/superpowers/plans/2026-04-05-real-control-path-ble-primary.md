# Real Control Path BLE-Primary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make BLE the packaged preferred control path by adding a bundled BLE helper with richer lifecycle commands, helper auto-resolution, reconnect-oriented control phases, and host-visible BLE diagnostics without requiring shell setup.

**Architecture:** Add a new packaged helper binary that owns BLE lifecycle state and bond metadata persistence, then extend `plugin-control-ble` to auto-resolve that helper, translate richer helper states into the shared control contract, and expose those states to the existing host/runtime diagnostics surface. Keep the current plugin architecture and fallback control plugin, but make BLE helper state explicit and packaged.

**Tech Stack:** Rust, existing workspace plugin/runtime crates, JSON-over-stdio helper contract, Cargo tests, release packaging

---

## File Structure

- Create: `helpers/ble-helper/Cargo.toml`
  - new helper binary crate
- Create: `helpers/ble-helper/src/main.rs`
  - helper CLI and persistent lifecycle state machine
- Modify: `Cargo.toml`
  - add the new helper crate to the workspace
- Modify: `crates/contracts/src/control.rs`
  - extend control phases for BLE lifecycle and reconnect state
- Modify: `plugins/control-ble/src/backend.rs`
  - extend control session state representation
- Modify: `plugins/control-ble/src/helper_bridge.rs`
  - add `status`, `stop`, and `forget-bond` commands and richer helper payloads
- Modify: `plugins/control-ble/src/helper_config.rs`
  - resolve packaged helper automatically from repo/bundle layout
- Modify: `plugins/control-ble/src/main.rs`
  - translate richer helper states, auto-resolve packaged helper, call helper stop on shutdown
- Modify: `plugins/control-ble/tests/helper_contract.rs`
  - cover richer helper command contract
- Modify: `plugins/control-ble/tests/linux_probe.rs`
  - update helper resolution expectations
- Modify: `apps/host-desktop/src/runtime.rs`
  - carry richer control phase through runtime snapshot
- Modify: `apps/host-desktop/src/app.rs`
  - surface richer BLE diagnostics/checklist text
- Modify: `apps/host-desktop/tests/app_state.rs`
  - cover richer BLE diagnostics visibility
- Modify: `scripts/package_release.py`
  - stage the helper into bundle `helpers/`

### Task 1: Extend Shared Control State And Helper Contract

**Files:**
- Modify: `crates/contracts/src/control.rs`
- Modify: `plugins/control-ble/src/backend.rs`
- Modify: `plugins/control-ble/src/helper_bridge.rs`
- Modify: `plugins/control-ble/tests/helper_contract.rs`
- Test: `plugins/control-ble/tests/helper_contract.rs`

- [ ] **Step 1: Write the failing helper-contract tests**

Add tests for:
- `status` JSON round-trip
- `stop` command handling
- `forget-bond` command handling
- new helper phases such as `Pairing`, `BondedIdle`, and `ReconnectPending`

- [ ] **Step 2: Run the helper-contract tests to verify they fail**

Run: `cargo test -p plugin-control-ble helper_contract -- --nocapture`
Expected: FAIL with missing helper-bridge commands/structs and unsupported phase mapping.

- [ ] **Step 3: Extend the shared control phase enums**

Update:
- `ControlSessionPhase` in `crates/contracts/src/control.rs`
- `ControlSessionState` in `plugins/control-ble/src/backend.rs`

Add:
- `Pairing`
- `BondedIdle`
- `ReconnectPending`

Keep existing variants intact so older callers still compile.

- [ ] **Step 4: Extend helper bridge types and commands**

Add in `plugins/control-ble/src/helper_bridge.rs`:
- `BleHelperStatus`
- `run_status`
- `run_stop`
- `run_forget_bond`

Extend phase mapping and command execution to parse richer helper responses.

- [ ] **Step 5: Update helper contract tests and verify they pass**

Run: `cargo test -p plugin-control-ble helper_contract -- --nocapture`
Expected: PASS with richer helper command/state coverage.

- [ ] **Step 6: Commit**

```bash
git add crates/contracts/src/control.rs \
  plugins/control-ble/src/backend.rs \
  plugins/control-ble/src/helper_bridge.rs \
  plugins/control-ble/tests/helper_contract.rs
git commit -m "feat: extend ble helper control phases"
```

### Task 2: Add A Packaged BLE Helper Binary

**Files:**
- Create: `helpers/ble-helper/Cargo.toml`
- Create: `helpers/ble-helper/src/main.rs`
- Modify: `Cargo.toml`
- Test: `helpers/ble-helper/src/main.rs` via existing plugin helper-contract tests

- [ ] **Step 1: Write the failing packaged-helper expectation**

Add or update tests so the workspace expects:
- a real helper binary crate in the workspace
- `probe`, `prepare`, `status`, `execute`, `stop`, `forget-bond` commands
- persistent bond-state behavior through a helper-local state file

- [ ] **Step 2: Run the relevant tests to verify they fail**

Run: `cargo test -p plugin-control-ble ble_helper_prepare_returns_control_phase_and_checklist -- --exact`
Expected: FAIL if the new helper crate/types are not yet wired in.

- [ ] **Step 3: Add the helper crate to the workspace**

Create `helpers/ble-helper` and add it to the top-level workspace members.

Implement a minimal helper that:
- stores state in a JSON file under a helper-owned directory
- exposes the required commands
- returns a consistent state machine
- simulates BLE lifecycle transitions deterministically

This helper does not need true OS BLE transport in this first implementation pass, but it must behave like the packaged preferred control path and persist bond-state semantics correctly.

- [ ] **Step 4: Build and verify the helper crate**

Run: `cargo build -p ble-helper`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml helpers/ble-helper/Cargo.toml helpers/ble-helper/src/main.rs
git commit -m "feat: add packaged ble helper binary"
```

### Task 3: Auto-Resolve The Packaged Helper And Translate Richer BLE State

**Files:**
- Modify: `plugins/control-ble/src/helper_config.rs`
- Modify: `plugins/control-ble/src/main.rs`
- Modify: `plugins/control-ble/tests/linux_probe.rs`
- Modify: `plugins/control-ble/tests/windows_probe.rs`
- Test: `plugins/control-ble/tests/linux_probe.rs`
- Test: `plugins/control-ble/tests/windows_probe.rs`

- [ ] **Step 1: Write the failing helper-resolution tests**

Add tests for:
- repo-local helper resolution from sibling target dir
- bundle helper resolution from `<app-root>/helpers/ble-helper`
- env-var override still taking precedence when present

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test -p plugin-control-ble linux_probe -- --nocapture`
Expected: FAIL because helper resolution still depends only on `IOS_CONTROL_BLE_HELPER`.

- [ ] **Step 3: Implement packaged/repo-local helper resolution**

In `helper_config.rs`:
- keep env-var override support
- otherwise resolve:
  - repo-local sibling helper binary in `target/.../debug`
  - packaged sibling helper under `helpers/`

In `main.rs`:
- use the resolved helper for probe/prepare/status/execute/stop
- map richer helper states to shared contract phases
- call helper stop during plugin stop when a helper session exists

- [ ] **Step 4: Verify the targeted resolution tests pass**

Run: `cargo test -p plugin-control-ble linux_probe -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/control-ble/src/helper_config.rs \
  plugins/control-ble/src/main.rs \
  plugins/control-ble/tests/linux_probe.rs \
  plugins/control-ble/tests/windows_probe.rs
git commit -m "feat: auto-resolve packaged ble helper"
```

### Task 4: Package The Helper And Surface Richer BLE State In The Host

**Files:**
- Modify: `scripts/package_release.py`
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`
- Test: `apps/host-desktop/tests/app_state.rs`

- [ ] **Step 1: Write the failing host-state tests**

Add tests that assert:
- richer BLE phases such as `ReconnectPending` and `BondedIdle` surface in host diagnostics/checklists
- fallback reason stays visible when BLE is blocked

- [ ] **Step 2: Run the targeted host tests to verify they fail**

Run: `cargo test -p host-desktop runtime_snapshot_populates_control_checklist_and_operator_message -- --exact`
Expected: FAIL until richer control phases/notes are surfaced.

- [ ] **Step 3: Update release packaging to stage the helper**

Modify `scripts/package_release.py` so bundle output includes:
- `helpers/ble-helper[.exe]`

- [ ] **Step 4: Surface richer BLE state in the host UI model**

Update host app/device detail handling so:
- richer control phases remain visible in diagnostics
- helper-provided notes/checklist items appear in device detail
- BLE failure text remains visible even when fallback exists

- [ ] **Step 5: Run the targeted host tests to verify they pass**

Run: `cargo test -p host-desktop runtime_snapshot_populates_control_checklist_and_operator_message -- --exact`
Expected: PASS

- [ ] **Step 6: Run the full validation commands**

Run: `cargo test -p plugin-control-ble`
Expected: PASS

Run: `cargo test -p host-desktop`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add scripts/package_release.py \
  apps/host-desktop/src/app.rs \
  apps/host-desktop/tests/app_state.rs
git commit -m "feat: surface packaged ble helper state in host"
```

## Self-Review

Spec coverage:
- packaged helper binary: Task 2 and Task 4
- helper lifecycle commands and richer phases: Task 1
- packaged helper resolution: Task 3
- reconnect-oriented BLE state translation: Task 3 and Task 4
- visible fallback/diagnostic policy: Task 4

Placeholder scan:
- all tasks have concrete files, commands, and expected results
- no `TODO`, `TBD`, or vague “add tests later” steps remain

Type consistency:
- richer control phases are introduced before plugin and host code depends on them
- helper bridge command additions are defined before `main.rs` uses them
- packaging changes refer to the helper binary name introduced in Task 2

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-05-real-control-path-ble-primary.md`.

Given the explicit user request to implement in this session, proceed with Inline Execution using `executing-plans`.
