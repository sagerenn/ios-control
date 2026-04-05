# Auto Device Inventory Design

**Goal:** Make `host-desktop` automatically discover and display a real device inventory on startup, including partially discovered devices, without inventing readiness that has not been observed.

## Scope

This design adds a host-owned device inventory layer that aggregates multiple observable sources into one canonical inventory snapshot.

In scope:
- host-owned device inventory model under `apps/host-desktop`
- provider-based discovery from multiple observable sources
- inventory aggregation and merge rules
- partially discovered device rows in the host UI
- honest readiness/sessionability composition
- lightweight known-device persistence for historical enrichment
- tests for normalization, merge rules, and UI behavior

Out of scope:
- full BLE helper implementation
- guaranteed physical-device validation on Windows or Linux
- reconnect automation
- replacing session orchestration with inventory orchestration
- automatic session start from discovered rows

## Problem

Today, the desktop host can show startup readiness and can supervise active runtime sessions, but it still lacks a real device inventory.

Current gaps:
- the dashboard is still effectively session-driven
- paired Bluetooth devices, mirror-only devices, and historical known devices do not appear as unified inventory rows
- the host has no canonical way to represent “real but not startable yet”
- the UI cannot guide the operator from partial discovery toward a startable session

This leaves the app honest about startup backend status, but still incomplete as an operator-facing control console.

## Recommended Approach

Add a host-owned `device inventory` subsystem in `apps/host-desktop`.

The inventory subsystem should:
- collect raw observations from multiple providers
- normalize them into a shared device model
- merge overlapping observations when identity is strong
- preserve separate rows when evidence is weak
- compute readiness and sessionability from observed evidence only

This is preferred over pushing discovery directly into `session-orchestrator` because:
- startup inventory and live session supervision are related but distinct concerns
- the host UI needs partially discovered rows before a session exists
- provider-specific heuristics are easier to evolve in the host layer without widening orchestrator responsibilities too early

## Architecture

Add a new inventory area under `apps/host-desktop`.

Recommended modules:
- `inventory/model.rs`
  - canonical device record and readiness types
- `inventory/providers/mod.rs`
  - provider trait and provider result types
- `inventory/providers/bluetooth.rs`
  - Bluetooth-paired or Bluetooth-visible device provider
- `inventory/providers/mirror.rs`
  - capture helper and mirror/window source provider
- `inventory/providers/known_devices.rs`
  - persisted device history provider
- `inventory/aggregator.rs`
  - merge rules and snapshot construction

Host flow:
1. Startup bootstrap resolves runtime/component readiness.
2. Inventory providers run.
3. The host builds an `InventorySnapshot`.
4. The dashboard renders inventory rows regardless of whether a session exists.
5. Active sessions decorate inventory rows instead of replacing inventory entirely.

Session orchestration stays separate:
- inventory discovers and describes devices
- session orchestration starts and supervises active sessions

## Device Model

The canonical host device record must support incomplete discovery.

Recommended fields:
- `inventory_id`
  - host-generated stable identifier for the current inventory row
- `display_name`
  - best human-facing label
- `identifiers`
  - Bluetooth address or system device ID when available
  - mirror/capture source ID when available
  - persisted known-device ID when available
- `evidence_sources`
  - which providers contributed evidence
- `capture_state`
  - what capture evidence exists
- `control_state`
  - what control evidence exists
- `sessionability`
  - whether a session can be started now
- `reasons`
  - operator-facing missing-requirement or blocked-state reasons
- `last_seen_at`
  - timestamp of the freshest live evidence

Recommended readiness enums:
- `Unavailable`
- `Discovered`
- `Ready`
- `Blocked(String)`

Recommended sessionability enums:
- `NotStartable`
- `StartableWithPreferredPath`
- `StartableWithFallback`
- `Unknown`

Critical rule:
- no readiness field may be upgraded based on guesswork or historical memory alone

Examples:
- Paired iPhone, no capture path:
  - `control_state = Discovered`
  - `capture_state = Blocked("no capture path observed")`
  - `sessionability = NotStartable`
- Active mirror, no BLE path, fallback ready:
  - `capture_state = Ready`
  - `control_state = Ready`
  - `sessionability = StartableWithFallback`
- Historical known device only:
  - visible in inventory
  - explicitly marked as historical and not currently reachable

## Provider Contract

Each provider should emit raw observations, not final UI rows.

Recommended provider output:
- provider name
- observation timestamp
- candidate identifiers
- display name if known
- observed capabilities
- observed blockers
- confidence level when identity is weak

The provider interface should allow:
- synchronous or asynchronous probing
- empty results without error when a provider is unsupported on the current host
- non-fatal degraded results when a provider probe fails

Provider failures must not collapse the entire inventory refresh.

## V1 Providers

### Bluetooth Provider

Purpose:
- report paired or otherwise visible Bluetooth phone-like devices

Allowed evidence:
- device name
- hardware/system identifier
- paired state
- transport visibility

Not allowed to claim:
- capture readiness
- startable session state by itself

### Mirror Provider

Purpose:
- report active mirror/window capture sources or capture-helper-known sources

Allowed evidence:
- source ID
- display name
- capture readiness
- active or inactive mirror state

Not allowed to claim:
- BLE readiness by itself

### Known-Device Provider

Purpose:
- enrich inventory with previous successful device records or persisted preferences

Allowed evidence:
- previous device identifiers
- previous display names
- previous preferred paths

Not allowed to claim:
- current live reachability
- current readiness

### Future Providers

The design should leave room for:
- direct receiver provider
- native platform discovery providers
- richer helper-provided inventory sources

The aggregator contract should not need redesign when new providers are added.

## Merge Rules

The aggregator should merge provider observations into one canonical row when identity is strong.

Strong merge conditions:
- exact match on stable system identifier
- exact match on persisted known-device identifier with live confirmation
- exact match on mirror source already linked to a known device record

Weak evidence rules:
- name similarity alone is not enough
- if identity is weak, keep separate rows instead of risking a bad merge

Priority rules:
- stable live identifiers beat names
- live observations beat historical known-device data
- positive evidence from one provider is not erased by missing evidence from another provider
- historical data may enrich labels and hints, but must not upgrade readiness

Examples:
- Bluetooth provider and known-device provider report the same stable device ID:
  - merge into one row
- Mirror provider reports `Operator Mirror` and historical data has `John’s iPhone`, but no shared identifier:
  - keep separate rows
- Mirror provider and Bluetooth provider both map to the same persisted known-device link:
  - merge and compose readiness

## Readiness Composition

Inventory readiness should be composed centrally in the aggregator.

Recommended logic:
- capture readiness comes only from capture-related evidence
- control readiness comes only from control-related evidence
- sessionability is derived from the combination

Examples:
- capture ready + preferred control ready:
  - `StartableWithPreferredPath`
- capture ready + fallback control ready:
  - `StartableWithFallback`
- control discovered only + no capture:
  - `NotStartable`
- historical-only row:
  - `Unknown` or `NotStartable`, depending on how explicit the host wants to be

All blocked or missing states should carry operator-facing reasons such as:
- `mirror not active`
- `no capture source observed`
- `BLE peripheral path unavailable`
- `window-input fallback unavailable`
- `known from history only`

## UI Behavior

The dashboard should become `inventory + sessions`, not only `active sessions`.

Startup behavior:
- run inventory automatically after bootstrap capability probing
- show all discovered rows, including partial and historical rows
- refresh inventory periodically and on explicit `Retry Detection`

Row behavior:
- each row shows:
  - display name
  - evidence badges
  - readiness summary
  - start action state

Recommended badges:
- `Bluetooth`
- `Mirror`
- `Known`
- `Capture Ready`
- `Control Ready`
- `Fallback`
- `Blocked`

Examples:
- `iPhone 15 Pro | Bluetooth | capture missing | Not startable`
- `Operator Mirror | Mirror | Capture Ready | Fallback | Startable`
- `John’s iPhone | Known | waiting for live source`

Interaction rules:
- selecting a partially discovered row updates the detail panel
- detail panel explains:
  - what was observed
  - what is missing
  - what action is needed to become startable
- start action is enabled only when the row is truly startable

Active session behavior:
- active sessions decorate the corresponding inventory row
- inventory refresh must not delete active-session state
- active-session status is additional row state, not a separate inventory universe

## Persistence

Keep lightweight known-device persistence in host-owned storage.

Persist:
- stable identifiers
- last chosen capture/control preference
- display name override if needed
- last successful session timestamp

Do not treat persisted records as proof of current availability.

Historical behavior:
- persisted data may create a `Known` row
- that row must remain visibly historical until live evidence arrives
- live evidence may upgrade the row, but persistence alone may not

This keeps known-device history useful without lying about current runtime state.

## Inventory Refresh Model

Inventory refresh should be separate from session refresh.

Recommended behavior:
- startup inventory scan after bootstrap
- periodic background refresh on a safe interval
- manual `Retry Detection` action
- provider-level timeout or degradation handling so one bad provider does not block the whole inventory pass

Inventory refresh should:
- update or remove partial rows when live evidence changes
- preserve active-session decoration
- avoid UI thrash where possible by using stable inventory IDs and deterministic merge ordering

## Integration Points

Expected host touch points:
- `apps/host-desktop/src/app.rs`
  - inventory state, refresh loop, and dashboard/detail wiring
- `apps/host-desktop/src/panels/dashboard.rs`
  - render discovered device rows instead of session-only rows
- `apps/host-desktop/src/view_models/fleet.rs`
  - shift from session-only rows to inventory-backed rows
- `apps/host-desktop/src/preferences.rs`
  - persist known-device history if kept host-local
- new inventory modules under `apps/host-desktop/src/inventory/`

The existing runtime/session path should consume selected inventory records, but not own provider discovery in this slice.

## Testing

Add focused coverage for:

### Unit Tests
- provider output normalization
- strong and weak merge behavior
- readiness composition
- historical row handling

### App-State Tests
- partially discovered devices appear in fleet
- blocked devices disable start action
- detail panel shows missing requirements
- historical known-device rows stay visibly non-live
- live observations override stale known-device-only state

### Acceptance Criteria For This Slice
- app can show Bluetooth-only, mirror-only, and known-only rows honestly
- overlapping observations merge when identity is strong
- weak evidence does not collapse separate devices into one row
- start remains disabled unless capture and control evidence actually support it
- active sessions remain visible alongside non-active discovered rows

## Tradeoffs

Why host-owned inventory instead of orchestrator-owned inventory:
- inventory exists before sessions
- UI needs partial rows and guidance before the runtime starts
- provider evolution is easier to isolate in the host app

Downside:
- some discovery-related logic stays above the orchestrator
- later runtime-wide inventory APIs may require refactoring

That tradeoff is acceptable for this slice because the immediate problem is operator-visible discovery, not distributed runtime supervision.

## Explicit Non-Claims

Completing this slice does not mean:
- real BLE control is fully implemented
- physical-device end-to-end Windows validation is done
- reconnect or recovery is complete

Completing this slice does mean:
- the app can show a real, honest, automatically refreshed device inventory
- partially discovered devices are visible and explained
- the host no longer needs to pretend that only active sessions exist
