# Auto Device Inventory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a real host-owned device inventory that automatically discovers partial devices, merges observations honestly, and renders inventory-backed fleet rows in `host-desktop`.

**Architecture:** Introduce an inventory subsystem under `apps/host-desktop` with provider-specific observations, a merge/aggregation layer, and a host app integration layer that decorates rows with active-session state. Keep inventory separate from session orchestration: providers describe what is observable, while the existing runtime still starts and supervises sessions.

**Tech Stack:** Rust, eframe/egui, Tokio, existing plugin protocol/runtime crates, host preferences JSON storage, Cargo tests

---

## File Structure

- Create: `apps/host-desktop/src/inventory/mod.rs`
  - inventory module exports
- Create: `apps/host-desktop/src/inventory/model.rs`
  - canonical inventory types, readiness, and evidence models
- Create: `apps/host-desktop/src/inventory/aggregator.rs`
  - merge rules, readiness composition, and snapshot building
- Create: `apps/host-desktop/src/inventory/providers/mod.rs`
  - provider trait/helpers and provider result types
- Create: `apps/host-desktop/src/inventory/providers/bluetooth.rs`
  - Windows Bluetooth discovery plus deterministic test override support
- Create: `apps/host-desktop/src/inventory/providers/mirror.rs`
  - capture-plugin-backed mirror source discovery
- Create: `apps/host-desktop/src/inventory/providers/known_devices.rs`
  - preferences-backed historical device rows
- Modify: `apps/host-desktop/src/preferences.rs`
  - persist known device records
- Modify: `apps/host-desktop/src/lib.rs`
  - export inventory module
- Modify: `apps/host-desktop/src/view_models/fleet.rs`
  - switch from session-only rows to inventory-backed rows
- Modify: `apps/host-desktop/src/view_models/device_detail.rs`
  - show inventory evidence, readiness, and reasons
- Modify: `apps/host-desktop/src/panels/dashboard.rs`
  - render inventory badges and blocked/startable states
- Modify: `apps/host-desktop/src/panels/device_detail.rs`
  - render inventory notes and missing requirements
- Modify: `apps/host-desktop/src/app.rs`
  - maintain inventory snapshot, refresh loop, and inventory/session decoration
- Modify: `apps/host-desktop/tests/support/mod.rs`
  - inventory test helpers and env guard helpers
- Create: `apps/host-desktop/tests/inventory_aggregator.rs`
  - provider merge/readiness tests
- Modify: `apps/host-desktop/tests/fleet_view_model.rs`
  - inventory row rendering tests
- Modify: `apps/host-desktop/tests/app_state.rs`
  - inventory-backed app-state tests
- Modify: `apps/host-desktop/tests/host_preferences_exact_names.rs`
  - preferences round-trip with known devices

### Task 1: Add Inventory Model, Providers, And Aggregation

**Files:**
- Create: `apps/host-desktop/src/inventory/mod.rs`
- Create: `apps/host-desktop/src/inventory/model.rs`
- Create: `apps/host-desktop/src/inventory/aggregator.rs`
- Create: `apps/host-desktop/src/inventory/providers/mod.rs`
- Create: `apps/host-desktop/src/inventory/providers/bluetooth.rs`
- Create: `apps/host-desktop/src/inventory/providers/mirror.rs`
- Create: `apps/host-desktop/src/inventory/providers/known_devices.rs`
- Modify: `apps/host-desktop/src/lib.rs`
- Modify: `apps/host-desktop/src/preferences.rs`
- Create: `apps/host-desktop/tests/inventory_aggregator.rs`
- Test: `apps/host-desktop/tests/inventory_aggregator.rs`

- [ ] **Step 1: Write the failing aggregator tests**

Add `apps/host-desktop/tests/inventory_aggregator.rs` with coverage for:
- merging Bluetooth + known-device observations on exact stable ID match
- keeping mirror-only and known-only observations separate when evidence is weak
- composing `StartableWithFallback` when mirror capture is ready and fallback control is ready
- keeping historical known-device rows non-live

- [ ] **Step 2: Run the aggregator tests to verify they fail**

Run: `cargo test -p host-desktop --test inventory_aggregator -- --nocapture`
Expected: FAIL with unresolved `inventory` imports and missing aggregation types/functions.

- [ ] **Step 3: Implement the minimal inventory model**

Define:
- `InventoryEvidenceSource`
- `CapabilityState`
- `Sessionability`
- `DeviceObservation`
- `InventoryDevice`
- `InventorySnapshot`

Keep the initial model small and explicit:
- stable IDs are optional
- reasons are flat `Vec<String>`
- historical rows carry a visible `live: bool`

- [ ] **Step 4: Implement the minimal providers**

Implement three providers:
- Bluetooth:
  - supports deterministic test input from `IOS_CONTROL_TEST_BLUETOOTH_DEVICES_JSON`
  - on Windows, best-effort discovery via PowerShell
  - on other hosts, returns no live rows by default
- Mirror:
  - uses the existing capture plugin path
  - lists capture sources when the plugin/helper path is actually usable
  - does not claim BLE readiness by itself
- Known devices:
  - reads `HostPreferences`
  - emits historical-only rows

- [ ] **Step 5: Implement the aggregator and readiness composition**

Merge rules:
- exact stable identifier match merges
- exact known-device link with live confirmation merges
- name-only evidence never merges

Readiness:
- capture readiness from mirror/capture observations only
- control readiness from Bluetooth/fallback evidence only
- sessionability composed centrally

- [ ] **Step 6: Run the aggregator tests to verify they pass**

Run: `cargo test -p host-desktop --test inventory_aggregator -- --nocapture`
Expected: PASS with all inventory merge/readiness tests green.

- [ ] **Step 7: Commit**

```bash
git add apps/host-desktop/src/inventory \
  apps/host-desktop/src/lib.rs \
  apps/host-desktop/src/preferences.rs \
  apps/host-desktop/tests/inventory_aggregator.rs
git commit -m "feat: add host device inventory model"
```

### Task 2: Render Inventory-Backed Fleet Rows And Detail State

**Files:**
- Modify: `apps/host-desktop/src/view_models/fleet.rs`
- Modify: `apps/host-desktop/src/view_models/device_detail.rs`
- Modify: `apps/host-desktop/src/panels/dashboard.rs`
- Modify: `apps/host-desktop/src/panels/device_detail.rs`
- Modify: `apps/host-desktop/tests/fleet_view_model.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`
- Test: `apps/host-desktop/tests/fleet_view_model.rs`
- Test: `apps/host-desktop/tests/app_state.rs`

- [ ] **Step 1: Write the failing fleet/detail tests**

Add tests that assert:
- partially discovered Bluetooth-only rows appear in `FleetViewModel`
- blocked rows keep start disabled and expose reasons
- detail panel state shows observed evidence and missing requirements
- live inventory rows beat historical known-only rows for display

- [ ] **Step 2: Run the targeted fleet/app-state tests to verify they fail**

Run: `cargo test -p host-desktop fleet_view_model_preserves_operator_actions_per_device -- --exact`
Expected: FAIL once assertions are updated to inventory-backed row fields.

Run: `cargo test -p host-desktop host_app_displays_partially_discovered_inventory_rows -- --exact`
Expected: FAIL because app state is still runtime-session-driven.

- [ ] **Step 3: Update fleet and detail view models**

Extend fleet rows with:
- evidence badges
- readiness label
- startable flag
- blocked reason
- active-session decoration

Extend device detail with:
- evidence lines
- inventory reason lines
- historical/live indicator

- [ ] **Step 4: Update dashboard and detail rendering**

Dashboard:
- render inventory rows, not just session rows
- show evidence badges and readiness summary

Detail panel:
- show observed evidence
- show missing requirements
- keep capture source selection only when a live session/workspace exists

- [ ] **Step 5: Run the targeted fleet/detail tests to verify they pass**

Run: `cargo test -p host-desktop fleet_view_model_preserves_operator_actions_per_device -- --exact`
Expected: PASS with updated inventory-backed rows.

Run: `cargo test -p host-desktop host_app_displays_partially_discovered_inventory_rows -- --exact`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add apps/host-desktop/src/view_models/fleet.rs \
  apps/host-desktop/src/view_models/device_detail.rs \
  apps/host-desktop/src/panels/dashboard.rs \
  apps/host-desktop/src/panels/device_detail.rs \
  apps/host-desktop/tests/fleet_view_model.rs \
  apps/host-desktop/tests/app_state.rs
git commit -m "feat: render inventory-backed host fleet"
```

### Task 3: Integrate Inventory Refresh Into Host App And Preferences

**Files:**
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/src/preferences.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`
- Modify: `apps/host-desktop/tests/host_preferences_exact_names.rs`
- Modify: `apps/host-desktop/tests/support/mod.rs`
- Test: `apps/host-desktop/tests/app_state.rs`
- Test: `apps/host-desktop/tests/host_preferences_exact_names.rs`

- [ ] **Step 1: Write the failing integration tests**

Add tests that assert:
- startup inventory runs automatically for `with_runtime` and `with_runtime_and_preferences`
- historical known-device rows round-trip through preferences
- successful runtime sessions enrich known-device history
- active sessions decorate inventory rows without deleting non-active discovered rows

- [ ] **Step 2: Run the targeted integration tests to verify they fail**

Run: `cargo test -p host-desktop host_app_merges_runtime_sessions_with_inventory_rows -- --exact`
Expected: FAIL because the app does not yet maintain a separate inventory snapshot.

- [ ] **Step 3: Add host app inventory state and refresh loop**

Implement:
- `inventory_snapshot` field in `HostDesktopApp`
- startup inventory refresh in `with_runtime`
- periodic inventory refresh separate from runtime frame refresh
- inventory-to-fleet sync
- active-session decoration on matching inventory rows

- [ ] **Step 4: Extend preferences with known-device history**

Add host-owned known-device records to `HostPreferences`:
- stable IDs
- last display name
- last selected source
- last successful timestamp if available

Keep backward compatibility:
- old JSON without `known_devices` still loads
- persistence remains non-fatal on bad or missing files

- [ ] **Step 5: Run the targeted app/preferences tests to verify they pass**

Run: `cargo test -p host-desktop host_app_merges_runtime_sessions_with_inventory_rows -- --exact`
Expected: PASS

Run: `cargo test -p host-desktop host_preferences_roundtrip_json -- --exact`
Expected: PASS with extended preference schema.

- [ ] **Step 6: Run the full host suite**

Run: `cargo test -p host-desktop`
Expected: PASS with all host inventory, preferences, and runtime tests green.

- [ ] **Step 7: Commit**

```bash
git add apps/host-desktop/src/app.rs \
  apps/host-desktop/src/preferences.rs \
  apps/host-desktop/tests/app_state.rs \
  apps/host-desktop/tests/host_preferences_exact_names.rs \
  apps/host-desktop/tests/support/mod.rs
git commit -m "feat: integrate auto device inventory into host app"
```

## Self-Review

Spec coverage:
- host-owned inventory subsystem: Task 1
- partial discovery and merge rules: Task 1
- inventory-backed UI rows and detail guidance: Task 2
- known-device persistence and inventory refresh: Task 3
- session decoration without collapsing inventory: Task 3

Placeholder scan:
- all tasks have concrete files, commands, and expected outcomes
- no `TODO`, `TBD`, or implied “write tests later” steps remain

Type consistency:
- `InventoryDevice`, `InventorySnapshot`, `CapabilityState`, `Sessionability`, and provider observation types are introduced before app/view-model tasks reference them
- preferences changes are isolated to `HostPreferences` and its tests before app integration depends on them

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-05-auto-device-inventory.md`.

Given the explicit user request to implement in this session, proceed with Inline Execution using `executing-plans`.
