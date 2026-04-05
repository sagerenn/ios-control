# Host Desktop Zero-Shell Bootstrap Design

**Goal:** Make `host-desktop` start cleanly with no shell setup, no required environment variables, and no manual plugin/helper path configuration for both `cargo run -p host-desktop` and packaged `host-desktop.exe` launches.

## Scope

This design covers bootstrap, runtime path resolution, startup capability probing, and guided startup UX for the host app.

In scope:
- automatic plugin/helper path resolution inside `host-desktop`
- one consistent startup path for repo launches and packaged app launches
- backend capability probing before any session starts
- an honest guided startup UI when the app cannot run a usable device flow yet
- packaging/layout rules needed for zero-shell startup
- tests for path resolution, capability normalization, and startup behavior

Out of scope:
- real device auto-discovery
- guaranteed physical iPhone/iPad control on Windows
- implementing a production BLE helper if one does not already exist
- inventing mock devices or auto-starting synthetic sessions by default
- changing the plugin architecture into a monolithic host binary

## Current Problem

Today, `host-desktop` still depends on runtime assumptions that are acceptable for developer experiments but not for a product-style launch:
- plugin paths are derived from Cargo target output assumptions
- helper paths are still primarily conveyed through environment variables
- startup behavior does not begin from an operator-facing readiness model
- the app can still imply a fake/default device state instead of surfacing the actual backend state

This makes the app fragile in both developer and packaged contexts and fails the requirement that the user should be able to start the app directly and be guided from inside the UI.

## Recommended Approach

Keep the current plugin-based runtime architecture, but move all bootstrap ownership into `host-desktop`.

`host-desktop` becomes responsible for:
- resolving runtime roots
- locating plugins and bundled helpers
- probing capabilities
- normalizing startup diagnostics
- launching the UI into a guided readiness state

Environment variables remain optional debug overrides only. They are never required for normal startup.

This is preferred over collapsing everything into one binary because it reaches zero-shell startup with a much smaller risk surface and preserves the existing plugin seams.

## Runtime Bootstrap Architecture

Add a focused bootstrap layer under `apps/host-desktop`.

Recommended modules:
- `bootstrap/runtime_locator.rs`
  - resolves the runtime root and concrete binary paths
- `bootstrap/capability_probe.rs`
  - probes known plugins/helpers and produces normalized startup state
- `bootstrap/model.rs`
  - shared startup structs such as path records, probe results, and readiness summaries

Expected host flow:
1. `main.rs` asks the bootstrap layer to resolve runtime paths.
2. The bootstrap layer probes available backends without starting a device session.
3. The bootstrap layer returns:
   - resolved plugin/helper paths
   - a capability snapshot
   - a normalized startup readiness state
4. The app constructs the runtime from the resolved internal paths.
5. The UI renders from the capability snapshot before any session is created.

The host runtime should never require shell-exported paths in order to initialize.

## Path Resolution

Add a `RuntimeLocator` that supports two primary launch contexts:
- `Workspace`
- `Bundle`

### Workspace Mode

This mode supports `cargo run -p host-desktop` with no manual setup.

Resolution rules:
- start from the current workspace root derived from `CARGO_MANIFEST_DIR`
- reuse the existing Cargo target-dir logic already present in host test support
- search:
  - `target/<build-target>/debug`
  - `target/debug`

Required plugin paths:
- `plugin-capture-window`
- `plugin-control-ble`
- `plugin-control-window-bridge`
- `plugin-grounding-core`

Workspace mode should not invoke Cargo or auto-build missing sibling binaries at runtime.
If those binaries are absent, startup must still succeed and enter a guided blocked state that explains which runtime components are missing.

If additional bundled helpers are later required, the locator should also search deterministic repo-local paths such as:
- `tools/`
- `dist/helpers/`
- another host-owned helper directory defined by the product layout

### Bundle Mode

This mode supports packaged `host-desktop.exe` launches with no manual setup.

Bundle layout contract:
- `<app-root>/bin/host-desktop.exe`
- `<app-root>/plugins/plugin-capture-window.exe`
- `<app-root>/plugins/plugin-control-ble.exe`
- `<app-root>/plugins/plugin-control-window-bridge.exe`
- `<app-root>/plugins/plugin-grounding-core.exe`
- `<app-root>/helpers/...` for any future non-plugin helper binaries

When launched from the packaged app:
- resolve `<app-root>` relative to the executable path
- prefer sibling `plugins/` and `helpers/` directories
- do not require `CARGO_TARGET_DIR`
- do not require any helper-related env vars

### Override Policy

Environment variables may still override individual paths for debugging or CI:
- `IOS_CONTROL_WINDOW_CAPTURE_HELPER`
- `IOS_CONTROL_WINDOW_INPUT_HELPER`
- `IOS_CONTROL_BLE_HELPER`

But overrides are secondary behavior only:
- normal startup must work without them
- the app must not tell operators to set them as part of normal setup

## Startup Capability Model

Before any session starts, the host should build a `HostCapabilitySnapshot`.

Recommended fields:
- resolved runtime root
- resolved plugin paths
- resolved helper paths
- binary existence status
- probe success or failure per backend
- normalized readiness state
- operator-facing reason strings

Readiness should be explicit:
- `Ready`
  - at least one usable capture path and one usable control path are available
- `Partial`
  - the app can launch and some paths work, but preferred or secondary paths are unavailable
- `Blocked`
  - the app launched successfully, but there is no usable end-to-end path yet

The host should probe at minimum:
- window capture backend
- BLE control backend
- window-input fallback control backend
- any future device-discovery component when it exists

The resulting snapshot should be the single source of truth for startup UI and diagnostics.

## Startup UX

The host must stop opening into a fake/demo session by default.

On launch, the app should show a readiness view driven by the capability snapshot:
- backend readiness summary
- exact missing or degraded components
- next-step guidance generated from the actual probe results
- a retry action for re-running capability detection

The startup screen should clearly communicate items such as:
- window capture ready / missing / probe failed
- BLE control ready / unsupported / missing helper
- window-input fallback ready / missing / unusable
- device discovery unavailable or not yet implemented

Critical UX rules:
- do not invent a device row unless a real session exists
- do not auto-start a mock session
- do not crash on missing binaries
- do not reduce startup failure to opaque shell-style errors

Instead, the app should guide the user from inside the UI with concrete, backend-specific messages.

## Startup Data Flow

The fixed startup sequence should be:

1. Resolve launch mode and runtime root.
2. Resolve plugin/helper paths.
3. Probe backend availability.
4. Normalize probe output into a capability snapshot.
5. Construct host runtime configuration from the internally resolved paths.
6. Render the startup readiness UI.
7. Only create a session after the operator explicitly starts one.

This ordering matters:
- path resolution errors should not be confused with probe failures
- probe failures should not be confused with session startup failures
- the operator should be able to understand what is missing before attempting a session

## Packaging Rules

Release packaging should be treated as a product runtime layout, not merely a loose binary archive.

Requirements:
- bundled plugins must be staged in a deterministic location the app can resolve automatically
- any future non-plugin helper binaries must be staged in a deterministic helper directory
- the packaged layout must match the runtime locator contract exactly

For developer launches, the repo layout must also be treated as a supported first-class runtime layout.

That means:
- `cargo run -p host-desktop` must succeed with zero manual env-var setup
- the repo build flow must produce binaries in locations that the runtime locator can discover automatically

For clarity:
- zero-shell startup does not imply implicit runtime compilation during app launch
- if required repo-local binaries are missing, the app launches and guides instead of failing

## Failure Handling

Failure handling must remain non-fatal at app startup.

Cases:

### Missing Binary

If a plugin/helper path cannot be resolved:
- mark that backend unavailable
- keep the app running
- include the expected path in diagnostics

### Probe Failure

If a binary exists but its probe fails:
- mark that backend degraded or unavailable
- surface the actual probe reason in diagnostics
- keep startup usable

### Preferred Backend Unavailable

If BLE is unavailable but window-input fallback is available:
- report the preferred backend failure explicitly
- keep fallback visible as the active usable path
- do not silently hide the BLE failure

### No Usable End-To-End Path

If no capture/control combination is usable:
- enter `Blocked`
- show exact next steps inside the UI
- do not fabricate a session or device

## Integration Points

Expected touch points:
- `apps/host-desktop/src/main.rs`
  - replace direct plugin-path construction with bootstrap-driven resolution
- `apps/host-desktop/src/app.rs`
  - add readiness-driven startup UI state
  - remove fake default product behavior from normal launch flow
- `apps/host-desktop/src/runtime.rs`
  - accept bootstrap-resolved runtime config without depending on shell setup
- `scripts/package_release.py`
  - enforce the runtime layout expected by bundle mode
- host tests under `apps/host-desktop/tests/`
  - add coverage for repo-mode and bundle-mode startup

## Testing

Add focused coverage for:

### Unit Tests
- workspace-mode path resolution
- bundle-mode path resolution
- env-var override precedence
- capability normalization into `Ready`, `Partial`, and `Blocked`

### Integration Tests
- repo launch path resolution works without env vars
- staged bundle launch path resolution works without env vars
- app startup with missing plugins enters guided `Blocked` state instead of crashing
- app startup with partial backend availability enters guided `Partial` state
- app startup with usable backends enters `Ready`

### Acceptance Criteria For This Slice
- `cargo run -p host-desktop` works with no env vars and no manual setup
- packaged `host-desktop.exe` works with no env vars and no manual setup
- startup never depends on shell-exported plugin/helper paths
- startup guidance is honest and backend-specific
- the app no longer defaults to a fake demo session/device state

## Tradeoffs

Why keep plugins instead of collapsing into one host binary:
- much smaller refactor
- preserves current runtime seams
- lower risk for ongoing development
- reaches zero-shell startup faster

Downside:
- the host bootstrap layer becomes more responsible for path resolution and diagnostics
- product packaging must stay aligned with the runtime locator contract

Those tradeoffs are acceptable for this slice because the primary problem is startup ownership, not plugin architecture itself.

## Explicit Non-Claims

Completing this slice does not mean the app is a complete physical-device product.

After this slice, the app should be able to:
- start cleanly without shell setup
- locate its own runtime components
- explain what is ready and what is missing

It will still need later work for:
- real device auto-discovery
- real validated end-to-end Windows iPhone workflows
- full product-grade BLE helper support if current helper coverage remains incomplete
