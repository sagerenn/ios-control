# Host Desktop Global Direct Receiver Design

**Goal:** Add a global `Start Direct Receiver` control to `host-desktop` so the app can launch a direct AirPlay-style receiver session from inside the desktop UI without requiring the operator to pre-launch an external helper or set shell environment variables.

## Scope

This design covers the host app UI, runtime configuration, bootstrap path resolution, and session-start wiring needed to launch a direct receiver from `host-desktop`.

In scope:
- a global host-level control for starting and stopping the direct receiver
- runtime/bootstrap support for resolving `plugin-capture-direct`
- explicit capture-backend selection when starting a session
- host-owned environment setup needed for the direct plugin to run its helper mode
- operator-visible readiness and error messages for the direct receiver path
- tests for bootstrap resolution, session start selection, and host UI state

Out of scope:
- a real device-discovery model for incoming AirPlay sessions
- redesigning inventory around a synthetic direct-receiver device row
- simultaneous multi-backend capture selection inside one session
- a full product-grade AirPlay interoperability claim
- replacing the existing window-capture workflow

## Current Problem

Today the host UI suggests a future direct-receiver path in docs and labels, but the actual desktop app does not expose a way to start it.

Current gaps:
- `host-desktop` only resolves `plugin-capture-window` as its capture backend
- startup readiness only probes the window-capture path
- the only visible session action is `Start Session`, which assumes an existing device/capture source flow
- the direct receiver plugin exists, but it is not wired into host bootstrap or runtime selection
- direct capture still depends on `IOS_CONTROL_DIRECT_RECEIVER_HELPER`, which is not configured by the host

This leaves the operator with no in-app action that corresponds to “start the receiver now.”

## Recommended Approach

Add a host-global direct-receiver workflow without disturbing the existing window-capture device flow.

The host app should gain:
- a global `Start Direct Receiver` action
- direct-receiver readiness/status text
- an internal way to start a session against `plugin-capture-direct`

The host runtime should gain:
- resolved paths for both capture plugins
- an explicit capture-backend selection field when a session starts
- a host-owned direct-session launch path that sets `IOS_CONTROL_DIRECT_RECEIVER_HELPER` to the resolved direct plugin executable before spawning the plugin

This is preferred over inventing a synthetic inventory row because the receiver is a host-wide service, not a discovered device. A global control matches the user’s mental model and requires a smaller change to current host state.

## Operator Experience

The host should expose a global direct-receiver section in startup and session-adjacent UI.

Expected operator flow:
1. Launch `host-desktop`.
2. See whether the direct receiver is available, blocked, or already running.
3. Click `Start Direct Receiver`.
4. The host starts a session using the direct capture backend.
5. The session view shows streaming preview and the selected direct source.
6. The operator uses `Stop Session` to stop the receiver session.

Expected UI behavior:
- if the direct backend is resolvable and probeable, enable `Start Direct Receiver`
- if the direct backend is missing or fails probe, disable the control and show the concrete reason
- if a direct session is active, do not show a second start action for that same receiver flow
- keep existing window-capture device actions unchanged

## Runtime Architecture

Extend the host runtime configuration so capture backend selection is explicit instead of implicit.

Recommended additions:
- `HostRuntimeConfig`
  - `window_capture_plugin_path`
  - `direct_capture_plugin_path`
  - existing control and grounding plugin paths
- `StartSessionRequest`
  - `capture_backend: CaptureBackend`
- new host-local enum:
  - `CaptureBackend::Window`
  - `CaptureBackend::Direct`

Session start behavior:
- `CaptureBackend::Window`
  - preserve the current flow and existing selected-source behavior
- `CaptureBackend::Direct`
  - spawn `plugin-capture-direct`
  - set `IOS_CONTROL_DIRECT_RECEIVER_HELPER` to the resolved direct plugin path for the child plugin process
  - use source `direct-1`
  - keep control backend selection unchanged

This keeps capture backend choice at the host/orchestrator boundary, which is the smallest stable seam for the change.

## Bootstrap And Path Resolution

The bootstrap layer must resolve both capture plugins.

Workspace layout requirements:
- `target/<target>/debug/plugin-capture-window`
- `target/<target>/debug/plugin-capture-direct`

Bundle layout requirements:
- `<app-root>/plugins/plugin-capture-window(.exe)`
- `<app-root>/plugins/plugin-capture-direct(.exe)`

Startup readiness should probe both capture paths independently:
- `Window Capture`
- `Direct Receiver`

Readiness summary rules:
- direct-receiver readiness should not replace the existing end-to-end readiness summary
- the UI should show the direct receiver as an independently explainable capability
- missing direct capture should not block the rest of the host if window capture still works

## Host State Model

The host app needs a small amount of global state for direct receiver status.

Recommended fields:
- direct receiver availability summary from startup probe
- whether the active session is using `CaptureBackend::Direct`
- direct receiver action error, if the last start attempt failed before a runtime status was produced

The direct receiver should not be represented as a fleet row unless a future design intentionally converts it into a synthetic inventory-backed object.

For this task, the global control owns the receiver lifecycle and the existing session view owns streaming state.

## Data Flow

Direct receiver start sequence:
1. Host bootstrap resolves `plugin-capture-direct`.
2. Startup probe evaluates direct capture capability.
3. The UI renders global direct-receiver readiness.
4. Operator clicks `Start Direct Receiver`.
5. Host calls runtime start with `CaptureBackend::Direct`.
6. Runtime starts a session using the direct capture plugin path.
7. The child plugin process receives `IOS_CONTROL_DIRECT_RECEIVER_HELPER=<direct-plugin-path>`.
8. The plugin uses its built-in helper mode to satisfy probe/stream operations.
9. The host renders the direct source preview and session metadata.

Direct receiver stop sequence:
1. Operator clicks `Stop Session`.
2. Host stops the active runtime session.
3. Session UI returns to idle.
4. Global direct-receiver controls return to a startable state if readiness still passes.

## Error Handling

The host must surface direct-receiver failures as operator-facing messages rather than silent disabled states.

Cases:

### Direct Plugin Missing

- show direct receiver as unavailable
- disable `Start Direct Receiver`
- include the resolved missing path in diagnostics/startup details

### Direct Plugin Probe Failed

- show direct receiver as blocked or error
- preserve the real probe message
- do not claim the receiver can start

### Direct Session Start Failed

- keep the app responsive
- show the exact runtime failure in session or direct-receiver diagnostics
- do not clear the window-capture state or unrelated device state

### Receiver Already Running

- while a direct session is active, suppress duplicate start attempts
- let `Stop Session` remain the single stop action

### Window Flow Still Available

- direct receiver failures must not block normal window-capture sessions
- the host should continue to show existing device rows and standard start behavior

## Testing

Add tests before implementation for the new behavior.

Required coverage:
- bootstrap/runtime locator resolves `plugin-capture-direct` in workspace and bundle layouts
- startup readiness reports a direct-receiver row/item separately from window capture
- host app exposes enabled/disabled global direct-receiver actions based on readiness
- starting the direct receiver chooses the direct capture backend instead of the window backend
- direct start propagates the expected helper path into the spawned plugin process environment
- stop behavior returns the UI to idle without breaking the existing window-capture flow

Existing window-capture tests should continue to pass without modification to their operator path.

## Integration Points

Expected files:
- `apps/host-desktop/src/bootstrap/runtime_locator.rs`
  - resolve direct capture plugin path
- `apps/host-desktop/src/bootstrap/capability_probe.rs`
  - probe/report direct receiver readiness
- `apps/host-desktop/src/runtime.rs`
  - add explicit capture-backend selection and direct-start path
- `apps/host-desktop/src/app.rs`
  - add global direct-receiver action handling and state wiring
- `apps/host-desktop/src/panels/startup.rs`
  - render direct-receiver status and start control
- `crates/session-orchestrator/src/lib.rs`
  - select capture plugin path from the requested backend

Tests are expected in:
- `apps/host-desktop/tests/app_state.rs`
- `apps/host-desktop/tests/bootstrap_locator.rs`
- any orchestrator test file needed for backend selection coverage

## Acceptance Criteria

This work is complete when all of the following are true:
- `host-desktop` shows a global `Start Direct Receiver` control
- the control is enabled only when the direct backend is actually startable
- clicking it starts a runtime session backed by `plugin-capture-direct`
- the resulting session shows `Direct Receiver` as the active source
- `Stop Session` cleanly tears the session down
- missing or broken direct-receiver prerequisites are surfaced with explicit in-app reasons
- existing window-capture startup and session flows still work
