# Host Desktop User Data Logs Design

## Goal

Persist host-desktop diagnostic logs to a `logs/` folder under the user's app data directory while preserving the current in-memory diagnostics UI.

## Design

- Reuse the existing host preferences path logic as the source of truth for the app data root.
- Derive the logs directory as a sibling of `host-preferences.json`.
- Create one log file per app launch.
- Append host-side diagnostic events to that file as they occur.
- Keep the app running if log file creation or writes fail; surface the failure through in-memory diagnostics and stderr.

## Scope

This change covers host-side startup, inventory, and session-start diagnostics only. It does not add plugin-side persistent log files or rotation/cleanup logic.

## File Layout

- `apps/host-desktop/src/preferences.rs`
  - expose helper(s) to derive the app data directory and logs directory from platform env or a preferences path
- `apps/host-desktop/src/logging.rs`
  - create and append to a per-launch host log file
- `apps/host-desktop/src/view_models/diagnostics.rs`
  - keep generating in-memory log lines used by the UI
- `apps/host-desktop/src/app.rs`
  - initialize the file logger when preferences-backed startup is available
  - mirror diagnostics events into the launch log file
- `apps/host-desktop/tests/app_state.rs`
  - verify launch log creation and append behavior
- `apps/host-desktop/tests/...`
  - verify path derivation helpers

## Behavior

- Windows target path: `%APPDATA%\ios-control\logs\host-desktop-<timestamp>-<pid>.log`
- Non-Windows target path: `<config-root>/ios-control/logs/host-desktop-<timestamp>-<pid>.log`
- Each launch gets a fresh file name based on current UTC epoch timestamp plus process id.
- Existing diagnostics panels continue to display recent log lines and counters from memory.

## Error Handling

- If the app cannot create the logs directory or log file, it does not fail startup.
- The app records a warning in diagnostics memory and emits a stderr warning.
- Later append failures are handled the same way.

## Testing

- Unit test logs directory derivation from a preferences path.
- Integration-style host app test that creates a temp preferences file, triggers diagnostics, and asserts:
  - `logs/` is created next to the preferences file
  - exactly one launch log file is created
  - the file contains expected diagnostic lines
