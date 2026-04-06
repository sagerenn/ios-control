# Real BLE Cross-Platform Backend Design

Date: 2026-04-06
Status: Design approved, awaiting written spec review

## Scope

This spec defines the concrete BLE implementation approach for real iPhone and iPad control from Linux and Windows hosts in this repository.

It narrows the problem to the real control transport:

- real BLE HID-style pairing and input delivery
- Linux and Windows support from the start
- typical built-in host adapters as the default target
- explicit visible fallback when BLE cannot be used

It does not redefine screen capture, grounding, or the higher-level session architecture already documented elsewhere.

## Context

The current repository already has:

- a shared Rust workspace
- a plugin boundary in `plugins/control-ble`
- a packaged helper shape in `helpers/ble-helper`
- a fallback control path in `plugins/control-window-bridge`
- design docs that already chose stock iOS BLE HID semantics instead of jailbreak-only or private API approaches

The remaining question is not whether the app should use Bluetooth in general. The remaining question is which implementation approach is suitable for real Linux and Windows support when the host must behave like a BLE HID peripheral that iOS can pair with.

## Constraints

- Target devices are stock iPhone and iPad devices.
- No jailbreak, private iOS API, or custom iOS app is assumed.
- Linux and Windows are both first-class targets.
- The first milestone must prove real BLE transport feasibility, not only architecture.
- Typical built-in adapters are in scope, but unsupported hardware must still be surfaced clearly.
- BLE remains the preferred path, but fallback to `control-window-bridge` must stay visible and explicit.
- V1 supports keyboard and pointer semantics, not arbitrary coordinate injection.

## Goals

- Make the host appear to iOS as a standard Bluetooth input device using BLE HID semantics.
- Keep a shared Rust action and HID layer across Linux and Windows.
- Isolate OS-specific BLE risk inside the packaged helper.
- Preserve a stable plugin contract for the host and orchestrator.
- Surface exact BLE failure reasons and fallback availability to the operator.

## Non-Goals

- Exact tap injection on arbitrary screen coordinates
- A single cross-platform BLE library that hides both operating systems
- Guaranteed support for every built-in adapter on day one
- Full CI proof of physical-device BLE behavior
- Replacing the plugin architecture with a monolithic host binary
- Introducing an external Bluetooth coprocessor or hardware bridge in V1

## Evaluated Approaches

### 1. Native OS Backends In Rust

Recommended.

Keep the existing helper and plugin split, share contracts and HID report logic in Rust, and implement two real BLE peripheral backends:

- Linux on top of BlueZ
- Windows on top of WinRT Bluetooth APIs

This matches the actual product requirement: the PC must behave like a BLE HID peripheral, not a BLE client.

### 2. Cross-Platform BLE Library First

Rejected for V1.

Libraries such as `btleplug`, Qt Bluetooth, and SimpleBLE do not provide a sufficiently strong fit for this product:

- `btleplug` is centered on BLE central/client workflows, which is the wrong role
- Qt Bluetooth does not give the required Windows peripheral path for this design
- SimpleBLE peripheral support is not mature enough to be the primary shipping path here

These libraries may still be useful for experiments, but they are not suitable as the core transport choice for this repository.

### 3. External Bridge Or Hardware Abstraction First

Rejected for V1.

Moving BLE into a separate daemon or external hardware path could improve long-term reliability, but it conflicts with the chosen product goal of supporting typical built-in adapters and would add deployment complexity before the real transport is proven.

## Recommended Stack

There is no single suitable popular Bluetooth library for this project. The suitable implementation is:

- shared Rust control and HID layers
- Linux-specific BLE peripheral backend
- Windows-specific BLE peripheral backend

Recommended concrete stack:

- shared crates:
  - `ios-control-contracts`
  - `ios-control-hid-report-engine`
  - helper lifecycle and error models in the existing helper bridge layer unless they become large enough to justify a dedicated crate
- helper runtime:
  - `tokio`
  - `serde`
  - `serde_json`
  - `thiserror`
  - `tracing`
- Linux:
  - `bluer` for BlueZ adapter and session primitives where it is sufficient
  - `zbus` directly for lower-level BlueZ D-Bus control where `bluer` does not expose the required GATT server or advertising behavior cleanly
- Windows:
  - `windows` crate for WinRT projections
  - specifically the Bluetooth adapter, GATT service provider, and advertising APIs needed for peripheral-role probing, service publication, and advertising

## Architecture

The implementation should keep the current repo shape and make the helper the real transport owner.

### Shared Layers

- `crates/contracts/src/control.rs`
  - normalized capability, control phase, and execution types
- `crates/hid-report-engine/src/lib.rs`
  - keyboard and pointer HID report generation from abstract actions

These layers remain OS-neutral.

### BLE Helper

`helpers/ble-helper` becomes the real BLE lifecycle owner.

Responsibilities:

- probe adapter and OS BLE peripheral support
- start and stop advertising
- expose the BLE HID-style service surface
- manage pairing and bonding state
- persist bond metadata
- attempt bounded reconnect
- accept abstract action execution requests and emit HID reports
- normalize transport-specific failures into helper-visible state

Recommended internal file split:

- `helpers/ble-helper/src/main.rs`
- `helpers/ble-helper/src/state.rs`
- `helpers/ble-helper/src/bond_store.rs`
- `helpers/ble-helper/src/hid_service.rs`
- `helpers/ble-helper/src/linux.rs`
- `helpers/ble-helper/src/windows.rs`
- `helpers/ble-helper/src/error.rs`

### Control Plugin

`plugins/control-ble` remains the stable plugin boundary.

Responsibilities:

- resolve the helper path without requiring shell configuration
- call helper lifecycle commands
- translate helper states into normalized plugin and contract states
- preserve exact BLE failure reasons
- expose fallback availability instead of silently downgrading

The plugin should not own OS BLE transport logic directly beyond lightweight probing and helper coordination.

### Host And Fallback

`apps/host-desktop` should present BLE as the preferred control path and `plugins/control-window-bridge` as the visible fallback path.

Fallback is allowed only after explicit BLE failure or bounded reconnect failure. The host must keep the BLE failure visible even when fallback is available.

## Runtime Flow

### Startup

1. `host-desktop` loads `plugin-control-ble`.
2. `plugin-control-ble` resolves and launches `ble-helper`.
3. `ble-helper` probes the radio, OS capability, and required peripheral/server features.
4. If the path is viable, the helper enters `ReadyToAdvertise`.
5. If the path is not viable, the helper enters `Unavailable` with a concrete reason.

### Pairing

1. The helper starts advertising as the BLE HID-style input device.
2. The iPhone or iPad pairs through iOS Bluetooth settings.
3. The helper stores bond metadata and transitions to `Connected`.

### Control Execution

Runtime action flow:

`host or planner -> plugin-control-ble -> ble-helper -> HID report engine -> OS BLE backend -> iOS`

The helper accepts abstract actions such as:

- `PointerMove`
- `PointerClick`
- `PointerScroll`
- `KeyPress`
- `TextEntry`

The helper converts them into HID reports and submits them over the active BLE connection.

### Reconnect

1. On later launches the helper checks for stored bond metadata.
2. The helper enters `ReconnectPending` and attempts bounded reconnect.
3. If reconnect succeeds, the helper returns to `Connected`.
4. If reconnect fails, the helper exposes the reason and offers re-pair or fallback.

### Fallback

If BLE is unavailable or reconnect fails beyond retry limits:

- BLE failure remains visible in the host UI
- fallback control is offered explicitly
- the system does not silently replace BLE with fallback

## Normalized State Model

The helper, plugin, and host should converge on one control state model:

- `Unavailable`
- `ReadyToAdvertise`
- `Advertising`
- `Pairing`
- `BondedIdle`
- `ReconnectPending`
- `Connected`
- `Error`

Every non-ready state must include:

- a concrete reason
- whether execute is currently allowed
- next operator action
- fallback availability where relevant

## Error Handling

Because the product targets typical built-in adapters, adapter variability must be treated as a normal runtime condition.

The helper should classify failures into at least these categories:

- peripheral role unsupported
- radio not detected
- BlueZ or WinRT backend unavailable
- advertising start failed
- GATT service registration failed
- pairing or bonding failed
- reconnect failed
- connected but execute blocked
- stale or corrupt bond metadata

The host should expose at least these operator actions:

- `Retry BLE`
- `Forget Bond And Re-pair`
- `Use Fallback Control`
- `View Diagnostics`

## File Boundaries

The current repo structure is already close to the correct split.

Recommended primary ownership:

- `crates/contracts/src/control.rs`
  - stable transport-agnostic control contract
- `crates/hid-report-engine/src/lib.rs`
  - shared report generation
- `helpers/ble-helper/src/*`
  - all real transport logic and persistent bond state
- `plugins/control-ble/src/helper_bridge.rs`
  - helper stdio contract
- `plugins/control-ble/src/helper_config.rs`
  - helper discovery and packaged resolution
- `plugins/control-ble/src/backend.rs`
  - normalized state model exposed to the host
- `apps/host-desktop/src/*`
  - operator-visible diagnostics, state, and fallback controls

This split keeps the risky BLE code isolated while preserving a stable host-facing interface.

## Validation Strategy

### Automated Validation

Automated coverage should include:

- unit tests for HID report generation
- unit tests for control-phase mapping and error normalization
- integration tests for the helper CLI contract
- integration tests for plugin helper resolution and lifecycle translation
- packaging tests that verify the helper is bundled correctly

### Manual Validation

Manual physical-device validation is required because CI cannot prove stock iOS BLE peripheral and HID behavior.

Linux success target:

- BlueZ present
- system bus available
- adapter present
- helper can advertise
- helper can register the GATT/HID surface
- iPhone or iPad can pair
- keyboard input works
- pointer input works

Windows success target:

- radio present
- peripheral role support reported
- helper can publish the GATT/HID surface
- helper can advertise
- iPhone or iPad can pair
- keyboard input works
- pointer input works

## Milestones

Recommended delivery order:

1. real capability probe on Linux and Windows
2. real advertising and service registration on Linux and Windows
3. first successful iOS pairing on Linux and Windows
4. keyboard input on Linux and Windows
5. pointer input on Linux and Windows
6. bond persistence and bounded reconnect
7. host diagnostics and explicit fallback polish

## Decision Summary

For this repository, the suitable implementation is not a single popular Bluetooth library.

The correct V1 choice is:

- shared Rust HID and control layers
- `bluer` plus `zbus` on Linux
- `windows` crate and WinRT Bluetooth APIs on Windows
- real transport isolated inside `helpers/ble-helper`
- visible fallback through `control-window-bridge`

This matches the existing plugin and helper architecture, keeps Linux and Windows aligned at the contract layer, and focuses engineering effort on the actual high-risk area: real BLE HID peripheral behavior on stock iOS from typical host hardware.
