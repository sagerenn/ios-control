# Host Preferences Persistence Design

**Goal:** Restore last-used host preferences on launch without automatically starting a session.

## Scope

This design adds lightweight, per-user persistence for host-side preferences only.

Persisted values:
- `selected_device_id`
- `selected_source_id`

Explicit non-goals for this change:
- automatic session restart on launch
- persistence of live session status
- persistence of plugin diagnostics, telemetry, or frame data
- cross-device sync
- real-device validation

## Approach

Use a small JSON preferences file owned by `apps/host-desktop`.

Storage location:
- Linux: XDG config directory, under an `ios-control` app directory
- Windows: `%APPDATA%` equivalent via the standard config-dir lookup

The host app remains the only consumer of this file. Lower-level crates such as `device-registry` stay in-memory for now.

## Data Model

Introduce a host-only preferences record:

```rust
pub struct HostPreferences {
    pub selected_device_id: Option<String>,
    pub selected_source_id: Option<String>,
}
```

Use a stable JSON representation with missing fields treated as `None`.

If the preferences file is missing, unreadable, or invalid JSON:
- do not crash
- return defaults
- optionally surface a non-fatal host diagnostic only if it helps local debugging

## File Ownership

Add a focused module under `apps/host-desktop` responsible for:
- resolving the config path
- loading preferences from disk
- saving preferences to disk

Keep file IO out of `app.rs` except for simple calls into that module.

## Launch Behavior

On app construction:
1. Load preferences from disk.
2. If `selected_device_id` is present, preselect that device in app state.
3. Do not auto-start a session.

When runtime-backed workspace data arrives:
1. If the saved `selected_device_id` matches an available runtime device, keep it selected.
2. If the saved `selected_source_id` exists among the current capture sources for that device, apply it as the active source.
3. If the saved source is unavailable for the selected device, clear only the saved source and continue normally.

If the saved device is unavailable:
- keep normal fallback behavior and select the first available device if one exists
- clear the persisted device value the next time preferences are saved from a valid selection

## Save Behavior

Write preferences when either of these changes:
- device selection changes
- capture-source selection changes

Write only the minimal JSON needed. Avoid writing on every frame refresh or runtime snapshot.

If saving fails:
- do not crash
- keep in-memory behavior intact
- surface a non-fatal host error only if it does not interfere with the current UI state

## Integration Points

Expected touch points:
- `apps/host-desktop/src/app.rs`
  - load preferences on startup
  - save after selection changes
  - reapply saved capture source when runtime workspace becomes available
- `apps/host-desktop/src/main.rs`
  - no automatic session start from persisted state
- new host preferences module
  - path resolution and JSON IO

## Validation Rules

Selection restore must be conservative:
- never invent a device if it is not present
- never invent a source if it is not present
- never auto-start a session

The persisted `selected_source_id` is meaningful only in the context of the current selected device. If the device changes, the source may need to be cleared.

## Testing

Add unit and app-state coverage for:
- preferences JSON roundtrip
- config path resolution returns a per-user path
- missing file returns defaults
- invalid JSON returns defaults without panicking
- app startup restores saved device selection without starting a session
- runtime workspace application restores a saved capture source when it is present
- unavailable saved source falls back safely
- changing device/source writes updated preferences

Keep existing `host-desktop` runtime and app-state suites green.

## Tradeoffs

Why host-only persistence instead of `device-registry` persistence:
- smaller change surface
- clearer ownership
- no need to introduce shared persistence semantics into lower-level crates yet

Downside:
- if reconnect/session persistence later expands, some of this logic may move downward into a shared registry or state store

That is acceptable for this scoped preference-restore feature.
