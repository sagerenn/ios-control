# Real Control Path BLE-Primary Design

**Goal:** Make the packaged BLE control path the preferred real product path, including first-time pairing, stored-bond reconnect, explicit operator-visible failure handling, and visible fallback behavior.

## Scope

This design covers the control-path architecture, BLE helper lifecycle, packaged helper resolution, reconnect semantics, and fallback policy for the real control path.

In scope:
- packaged BLE helper as the primary control transport owner
- zero-shell helper resolution for packaged and repo-local launches
- helper lifecycle commands beyond one-shot probe/prepare/execute
- normalized control state machine across helper, plugin, and host
- bond metadata storage and operator-visible reconnect behavior
- explicit fallback policy when BLE is unavailable or fails
- integration and manual validation requirements for paired plus reconnect behavior

Out of scope:
- full multi-device BLE production hardening
- all recovery scenarios beyond bounded reconnect
- claiming the entire app is product-complete
- replacing the plugin architecture with a monolithic host binary
- full physical-device validation for every supported host/platform combination

## Problem

Today, the repo has:
- a control plugin contract
- a minimal helper contract
- mock-oriented or partially helper-backed control behavior
- fallback control behavior through `control.window-bridge`

But it does not yet have a product-grade preferred BLE path that:
- is packaged with the app
- pairs a real device cleanly
- stores and reuses bond metadata
- reconnects in later launches
- exposes a live operator-facing control state
- falls back only in an explicit, reasoned, visible way

This is the primary gap between an experimental control architecture and a packaged BLE-first operator product path.

## Recommended Approach

Keep the current plugin architecture, but add a host-visible `control path manager` centered on a packaged BLE helper.

Responsibilities:
- `host-desktop`
  - show preferred control path status
  - show reconnect and fallback guidance
  - expose operator actions such as retry or forget-bond
- `plugin-control-ble`
  - remain the shared control backend contract
  - normalize helper state into shared control/session semantics
- packaged BLE helper
  - own OS-specific BLE transport behavior
  - advertise
  - pair and bond
  - store bond metadata
  - reconnect using stored metadata
  - execute HID-style actions
  - report normalized live control state

This is preferred over embedding all BLE transport directly in `plugin-control-ble` because:
- Windows BLE transport is the highest-risk area and benefits from isolation
- helper packaging and lifecycle management can evolve without destabilizing the shared plugin contract
- the host can reason about preferred/fallback control states at a higher level without carrying OS-specific transport code

## Architecture

The control path should be composed as:

- `host-desktop`
  - resolves helper path automatically
  - displays preferred path, fallback path, and operator actions
- `plugin-control-ble`
  - translates helper lifecycle/status into shared contract replies
  - keeps the plugin-facing control surface stable
- `helpers/ble-helper`
  - packaged runtime transport owner
  - performs real OS-specific BLE operations

Expected control flow:
1. Host/plugin resolve the packaged helper path automatically.
2. Plugin probes helper capability and runtime bond state.
3. Plugin starts or resumes BLE lifecycle through the helper.
4. Helper reports live state transitions.
5. Plugin exposes those transitions through the shared control-session model.
6. Host renders operator-facing control state and actions.

Fallback remains in scope, but only as a visible secondary path.
This slice treats BLE as the preferred path and the primary success target.

## Packaged Helper Layout

The helper must be packaged and resolved without shell setup.

Recommended runtime layout:
- `<app-root>/bin/host-desktop[.exe]`
- `<app-root>/plugins/plugin-control-ble[.exe]`
- `<app-root>/helpers/ble-helper[.exe]`

Repo-local dev layout should also be supported through deterministic resolution from the workspace.

Normal startup rules:
- do not require `IOS_CONTROL_BLE_HELPER`
- do not require the user to run the app from a special shell
- do not tell operators to configure helper env vars in standard setup docs

Environment variables may remain as debug overrides, but not as required runtime inputs.

## Helper Contract

The helper must move from a thin command shim to a real lifecycle owner.

Recommended commands:
- `probe`
  - report adapter capability
  - report BLE peripheral support
  - report helper version/build metadata
  - report whether a stored bond exists
- `prepare`
  - start or resume the control lifecycle
  - return current control phase and checklist/notes
- `status`
  - return current control phase and live metadata without reinitializing lifecycle
- `execute --plan-kind <kind>`
  - send real HID-style actions through the active BLE connection
- `stop`
  - gracefully stop advertising or transport state
- `forget-bond --device <id>`
  - remove stored bond metadata and force a clean re-pair

Recommended helper response fields:
- control phase
- paired or bonded device metadata if known
- whether the current state came from fresh pairing or reconnect
- whether execute is currently allowed
- failure reason if blocked
- checklist and notes for operator guidance

This contract is stronger than the current minimal helper contract because reconnect and operator-visible control state need a live status path, not only one-shot prepare/execute calls.

## Control State Machine

Define one normalized state machine across helper, plugin, and host.

Recommended states:
- `Unavailable`
  - helper or adapter cannot support the BLE path
- `ReadyToAdvertise`
  - BLE path is supported but not yet advertising
- `Advertising`
  - waiting for first-time pairing or reconnect
- `Pairing`
  - active pairing or bonding flow in progress
- `BondedIdle`
  - device is bonded but not currently connected
- `Connected`
  - device is connected and execute is allowed
- `ReconnectPending`
  - helper is trying to restore a known bond/session
- `Error`
  - control path failed and needs operator action or fallback

Key transitions:
- `ReadyToAdvertise -> Advertising`
- `Advertising -> Pairing`
- `Pairing -> Connected`
- `Connected -> BondedIdle`
- `BondedIdle -> ReconnectPending`
- `ReconnectPending -> Connected`
- any state -> `Error`
- `Error -> ReadyToAdvertise` or `Advertising` after operator retry/reset

Every non-ready state must carry:
- a concrete failure or waiting reason
- a next operator action
- fallback availability when relevant

## Bond Metadata And Reconnect

Stored bond metadata is required for the `paired + reconnect` success bar.

The helper should own:
- bond metadata persistence
- any helper-local device identifier used to resume a known bond
- the mapping between stored bond state and helper status

Requirements:
- successful first-time pairing stores enough bond metadata for later reconnect
- later launches should report whether a stored bond exists
- reconnect should be attempted explicitly through the helper lifecycle
- stale or corrupt bond state must be detectable and removable

Operator-visible actions:
- `Retry BLE`
- `Forget Bond And Re-pair`
- `Use Fallback Control`
- `View Diagnostics`

The operator must be able to force a clean re-pair without touching files or shell commands.

## Plugin-Control-BLE Responsibilities

`plugin-control-ble` remains the shared control backend contract.

It should:
- resolve the packaged helper automatically
- translate helper responses into normalized `ControlCapability`, `ControlSessionPhase`, and `ExecutionSummary`
- preserve the preferred BLE path as the first choice
- expose helper-originated failure reasons rather than collapsing them into generic errors
- retain fallback visibility when BLE cannot be used

It should not:
- hide BLE initialization failures when fallback is available
- silently downgrade from BLE to fallback without the host seeing the exact reason

The plugin is the translation boundary between transport-specific helper behavior and the stable control contract used by the host/orchestrator.

## Failure And Fallback Policy

BLE remains preferred, but failure handling must be explicit and bounded.

Failure categories:
- adapter capability failure
- helper startup failure
- advertising failure
- pairing or bonding failure
- reconnect failure
- execute-time transport failure
- stale or corrupt bond metadata

Host policy:
- prefer BLE whenever helper capability is real
- only offer fallback after:
  - explicit capability failure, or
  - bounded startup or reconnect failure beyond retry limits
- always surface:
  - why BLE failed
  - whether fallback is available
  - what operator action can restore BLE

Critical rules:
- fallback must never erase BLE failure evidence
- fallback choice must be visible in the UI
- reconnect attempts must be bounded
- the app must not hang forever in hidden retry loops

## Host UI Behavior

The host should expose the preferred control path clearly in the device/session detail area.

For BLE control, the host should show:
- current BLE state
- paired device identity when known
- whether the path is in first-pair or reconnect mode
- whether execute is currently allowed
- exact failure reason when blocked

When fallback is relevant, the host should show:
- fallback availability
- why fallback is being offered
- that BLE remains the preferred path

Recommended operator actions in the UI:
- `Retry BLE`
- `Forget Bond And Re-pair`
- `Use Fallback Control`
- `View Diagnostics`

The host must never leave the operator guessing whether it is still trying BLE, waiting for pairing, or already operating on fallback.

## Integration Points

Expected code touch points:
- `plugins/control-ble/src/main.rs`
  - extend helper lifecycle integration beyond the current minimal path
- `plugins/control-ble/src/helper_bridge.rs`
  - add helper command support for `status`, `stop`, and `forget-bond`
- `plugins/control-ble/src/helper_config.rs`
  - shift from env-var-required helper lookup to packaged helper resolution
- `apps/host-desktop`
  - render richer control state and operator actions
- release packaging scripts
  - stage the BLE helper in the deterministic helper location

The control-window fallback path remains separate, but this slice must make the BLE path clearly preferred and explicitly diagnosable.

## Testing

Add focused coverage for:

### Unit Tests
- helper command decoding
- control state transition normalization
- fallback decision policy
- bond-state error handling

### Integration Tests
- packaged helper resolution without env vars
- helper `probe/prepare/status/execute/stop/forget-bond` command contract
- plugin translation of helper live states into shared control states
- reconnect state propagation into host diagnostics

### Manual Validation
- Windows + first-time real iPhone pairing
- Windows + reconnect after app restart
- Windows + reconnect after disconnect/re-enable Bluetooth
- Linux equivalent path if the same scope is intended for Linux in this slice
- visible fallback offer when BLE is unsupported or fails

## Acceptance Criteria For This Slice

This slice is complete only if all of the following are true:
- packaged app starts BLE lifecycle without shell setup
- first-time pairing can reach `Connected`
- later launches can reach `ReconnectPending` and then `Connected`
- real execute path uses packaged BLE helper transport, not a mock path
- host surfaces exact BLE state and exact fallback reason
- operator can explicitly forget bond metadata and re-pair

## Tradeoffs

Why use a packaged helper plus control path manager instead of embedding all BLE logic directly in the plugin:
- isolates the highest-risk OS-specific transport logic
- keeps the shared plugin contract stable
- makes packaged helper lifecycle and diagnostics easier to reason about

Downside:
- one more packaged binary to manage
- more explicit lifecycle/state coordination across host, plugin, and helper

That tradeoff is acceptable because product-grade BLE behavior is inherently stateful and OS-specific, and hiding that complexity inside a thin plugin boundary would make failures harder to diagnose.

## Explicit Non-Claims

Completing this slice does not mean:
- multi-device BLE is fully hardened
- every recovery edge case is solved
- the entire app is complete

Completing this slice does mean:
- BLE is the real packaged preferred control path
- first-pair and reconnect are part of the normal operator flow
- fallback remains visible and honest rather than implicit
