# Host Desktop Bluetooth Launcher Direct Session Design

**Goal:** Redesign `host-desktop` so the main window shows only paired Bluetooth devices and read-only settings, while double-clicking a device opens or launches a direct-receiver-backed control session window.

## Scope

This design reshapes the desktop host into a launcher plus session-window model.

In scope:
- main window reduced to `Devices` and `Settings`
- device list limited to Bluetooth-paired inventory rows
- simplified device readiness as `Startable` or `Not Startable`
- double-click device launch behavior
- separate session window for waiting, blocked, error, and streaming states
- read-only settings panel with resolved file paths
- continued persistence of detailed diagnostics in log files instead of the main UI
- runtime changes needed to let direct sessions exist before mirroring is live
- tests for launcher filtering, launch behavior, and session-window state transitions

Out of scope:
- multiple simultaneous session windows in v1
- editable settings such as changing log directories
- a full redesign of inventory providers
- removing host-side logging or startup probes
- exposing plugin diagnostics in the root UI

## Current Problem

Today `host-desktop` is still a single-window operations console. It exposes dashboard, device detail, diagnostics, startup readiness, and settings information in one surface.

Current gaps relative to the target UX:
- the main window shows much more than the operator needs
- the root list includes non-Bluetooth inventory rows that should not appear in the launcher
- direct receiver launch is currently a global host action instead of a per-device action
- direct session start assumes a frame is available during startup, which does not match the desired “select device first, mirror later” flow
- detailed diagnostics are mixed into the visible UI rather than being treated as log data

## Recommended Approach

Keep the existing host runtime, bootstrap, inventory, preferences, and logging layers, but reshape the app into two explicit UI surfaces inside one process:

- a root launcher window
- a device session window

The launcher window should behave like a focused device picker:
- show only Bluetooth-paired devices
- show only a simple readiness label per row
- start a device-specific direct receiver session on double-click

The session window should own all session-specific states:
- waiting for the first mirrored frame
- blocked because prerequisites are missing
- startup or runtime error
- active streaming preview and control

This is preferred over preserving the current single-window composition because it matches the desired operator workflow while reusing most of the existing runtime and inventory code.

## Interaction Model

### Root Window

The root `host-desktop` window shows only two visible sections:
- `Devices`
- `Settings`

The root window does not render:
- dashboard summaries
- device detail panel
- startup readiness panel
- diagnostics panel
- plugin list
- in-memory host log lines

### Devices List

The `Devices` list is built from inventory rows that contain Bluetooth evidence.

Each row shows:
- device name
- readiness label: `Startable` or `Not Startable`

The launcher does not show mirror-only rows or historical-only rows even if they still exist in the full inventory snapshot.

### Double-Click Behavior

For a `Startable` device:
- double-click starts a direct receiver session associated with that Bluetooth device
- the session window opens only after the first mirrored frame is available

For a `Not Startable` device:
- double-click opens the session window immediately
- the session window shows the device and the missing reasons
- the launcher does not silently ignore the action

### Settings

The `Settings` section is read-only in v1.

It shows resolved file paths only:
- preferences/config path
- log directory path
- active launch log file path when available

## Architecture

### Process Model

Use one `host-desktop` process with multiple native windows rather than spawning a separate process per session window.

This keeps:
- runtime ownership centralized
- logging behavior unchanged
- preferences access in one place
- the change set smaller than a multi-process redesign

### Session Ownership

The host app keeps one active session in v1.

If the operator launches another device while a session is active, the app replaces the current session rather than managing multiple concurrent device windows. This is a deliberate v1 simplification.

### Device Identity

A session launched from the root window uses the selected Bluetooth device as the logical session device. The session start request should use:
- the selected Bluetooth inventory identity as `device_id`
- the selected Bluetooth device name as `device_name`
- `CaptureBackend::Direct` as the capture backend

This keeps the operator mental model aligned: they launch a specific paired device, not a generic global receiver row.

### Startability In The Launcher

Launcher `Startable` should be computed for the Bluetooth-launcher use case, not copied directly from the old window-capture inventory semantics.

A Bluetooth row is `Startable` when:
- Bluetooth evidence for the device exists
- the direct receiver backend is available on the host
- the device has a usable control path

It is `Not Startable` otherwise.

Mirror availability must not be required for the launcher row to become `Startable`, because mirroring is expected to begin after session launch.

## Runtime And Data Flow

### Startup

Startup behavior stays mostly intact:
- bootstrap resolves plugin paths
- startup probes run
- inventory providers refresh
- detailed probe output is written to logs

The root UI uses only the subset of that state needed for the launcher and settings panels.

### Startable Device Flow

1. Operator double-clicks a `Startable` Bluetooth device row.
2. Host starts a session for that device using the direct capture backend.
3. The session remains valid even if no mirrored frame exists yet.
4. The host polls until the first frame is available.
5. Once the first frame arrives, the session window becomes visible.
6. The session window then shows preview and control state for that device.

### Not Startable Device Flow

1. Operator double-clicks a `Not Startable` Bluetooth device row.
2. The app opens the session window immediately.
3. The session window shows a blocked state with concise missing requirements.
4. Detailed technical cause stays in log files.

### Runtime Change Needed

The current direct capture flow requires a frame during session startup. That must change.

Required behavior:
- the direct receiver path can create a live session before the first mirrored frame exists
- waiting for the first frame is represented as a valid session-window state rather than an immediate startup failure
- once the first frame arrives, the session transitions into normal streaming behavior

This waiting state is the key runtime change for the feature.

## Session Window Behavior

The session window is the only UI surface that shows per-device session state.

Required states:
- `Waiting for Mirror`
  - used when the device is startable and the receiver session has launched but no first frame exists yet
- `Blocked`
  - used when the selected device is not startable
- `Error`
  - used when runtime start or refresh fails
- `Streaming`
  - used when preview frames are available

The session window should provide:
- device name
- concise status line
- missing reasons when blocked
- preview image and current source details when streaming
- stop/close control

The root window should not mirror those details.

## Logging And Diagnostics

Detailed diagnostics remain important, but they move out of the visible root UI.

The app should continue to log:
- startup probe results
- inventory refresh results
- session start attempts
- session start failures
- session refresh failures

The operator-facing UI should only show concise reasons in the session window when needed. Full diagnostics continue to live in the host launch log file.

## File Layout

Recommended file responsibilities:

- `apps/host-desktop/src/app.rs`
  - host app orchestration
  - root-window and session-window state routing
  - double-click launch handling
- `apps/host-desktop/src/panels/launcher.rs`
  - render the Bluetooth-only launcher devices panel in the root window
- `apps/host-desktop/src/panels/session_view.rs`
  - render waiting, blocked, error, and streaming session-window states
- `apps/host-desktop/src/panels/settings.rs`
  - render read-only resolved paths instead of plugin rows
- `apps/host-desktop/src/view_models/fleet.rs`
  - provide launcher-ready Bluetooth-only rows with simplified readiness
- `apps/host-desktop/src/view_models/settings.rs`
  - provide resolved path rows for the settings panel
- `apps/host-desktop/src/view_models/session.rs`
  - model waiting-before-first-frame, blocked, error, and streaming states
- `crates/session-orchestrator/src/lib.rs`
  - allow direct sessions to start before the first frame is available
- `plugins/capture-direct/...`
  - keep the direct plugin compatible with a wait-for-first-frame session flow

## Error Handling

### Startup Or Probe Failures

- keep the app running
- record the details in logs
- reflect launcher startability honestly

### Not Startable Devices

- allow double-click
- open the session window in `Blocked`
- show concise missing requirements there

### Direct Session Start Failure

- open or keep the session window visible
- show `Error`
- do not fail silently

### Runtime Refresh Failure

- keep the session window open
- show the error state there
- preserve the log trail

## Testing

Required coverage:

- launcher view-model or app-state tests that show only Bluetooth-backed rows in the root window
- tests that compute launcher `Startable` independently of existing mirror visibility
- tests that double-clicking a `Startable` row delays session-window visibility until the first frame exists
- tests that double-clicking a `Not Startable` row opens the session window immediately in blocked state
- settings tests that surface resolved preferences and log paths
- runtime/orchestrator tests proving the direct path can enter a waiting state without failing session start
- regression tests that logging still records startup, inventory, and session failures even though diagnostics are hidden from the root UI

## Acceptance Criteria

This change is complete when all of the following are true:

- the root `host-desktop` window shows only `Devices` and `Settings`
- the root device list shows only Bluetooth-paired devices
- each device row shows only `Startable` or `Not Startable`
- double-clicking a `Startable` device launches a direct-receiver-backed session for that device
- the session window for a startable device opens only after mirroring becomes live
- double-clicking a `Not Startable` device opens a blocked session window immediately
- settings show resolved paths only
- detailed diagnostics are removed from the root UI and remain available in log files
- direct receiver session startup no longer fails just because the first frame is not yet available
