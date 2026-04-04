# TODO

Code-verified gap list for the current branch as of 2026-04-04.

This document is meant to be a living checklist. It reflects the current codebase, not the aspirational design docs under `docs/superpowers/`.

## Current Reality

- This repository is a Linux/Windows host workspace. There is no native iOS app target in this repo today.
- `apps/host-desktop` is still a shell UI. It is not wired directly to the session orchestrator or plugin runtime yet.
- The capture and control plugins are helper-driven. The current capture paths still synthesize frame payloads instead of transporting real device pixels end to end.
- Automated verification currently covers mock/plugin/contract flows. It does not cover a real iPhone/iPad end-to-end session.

## Product TODO

### 1. Wire the desktop host to the real runtime

- Add a real runtime layer to `apps/host-desktop` that owns session startup, status updates, and shutdown.
- Replace the in-memory `HostRuntimeBridge` with a bridge that talks to `ios-control-session-orchestrator`.
- Remove the fallback bootstrap error path and the simulated pending-start countdown from the host app.
- Feed real session state, capture source selection, plugin health, and operator actions into the UI.

### 2. Render a real live preview

- Transport actual frame bytes through the capture helpers/plugins instead of writing repeated fill bytes into frame slots.
- Consume frame-slot pixel data in the desktop host and render it as an image instead of a text-only frame summary.
- Support frame refresh, resize, rotation, and degraded/stalled capture states in the UI.
- Validate both capture paths separately:
  - window capture against a real mirrored window
  - direct receiver against a real iPhone/iPad screen-mirroring session

### 3. Complete real control execution

- Decide the supported production control path:
  - native BLE HID transport
  - helper-backed BLE transport with a stable helper contract
  - window-input fallback for mirrored-window operation
- Replace probe-only platform checks with a complete execution path that can be validated on real hardware.
- Surface control setup, advertising, pairing, failure, and reconnect states in the desktop UI.
- Verify that applied plans produce observable on-device effects, not just execution summaries.

### 4. Close the host/operator workflow gaps

- Let the operator choose devices and capture sources from real runtime data.
- Start and stop sessions from the UI with actual runtime side effects.
- Show actionable recovery guidance for degraded capture/control states.
- Persist enough session/device state to support reconnect and multi-device workflows cleanly.

### 5. Add real-device validation

- Run and record manual validation for:
  - Linux + window capture + BLE HID
  - Linux + window capture + window-input fallback
  - Windows + window capture + BLE HID
  - Windows + window capture + window-input fallback
  - any direct-receiver path that is intended to be supported
- Promote acceptance-matrix rows to `Verified` only after real operator validation.
- Add the strongest practical automated smoke coverage for real-runtime startup without depending on physical hardware in CI.

### 6. Keep status docs aligned with code

- Keep `README.md`, this file, and `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md` aligned with the current branch.
- Treat the design and plan docs in `docs/superpowers/` as historical planning artifacts unless the code and tests match them.
- Update the top-level docs immediately when a TODO item moves from mock-only to real verified behavior.

## Exit Criteria For "Complete App"

Do not describe this repository as a complete iOS remote-control app until all of the following are true:

- The desktop host starts and supervises real sessions through the orchestrator.
- The host renders a real live preview from a physical device or supported mirror path.
- Control actions are executed through a validated real-device path.
- Recovery and reconnect behavior are manually verified on supported platforms.
- The acceptance matrix records at least one fully verified real-device workflow.
