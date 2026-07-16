# TODO

Code-verified gap list for the current branch as of 2026-04-05.

## Bug review (2026-07-01)

Deep horizontal + vertical code review. Verified findings are recorded in
`docs/bug-review.md` (10 bugs: 4 High, 2 Medium, 4 Low). Tracker:

- [x] Review core crates (orchestrator, frame-transport, hid-report-engine, plugin-runtime, contracts)
- [x] Review plugins (capture-direct, capture-window, control-ble, control-window-bridge, grounding-core, mock-device)
- [x] Review helpers (ble-helper, direct-beacon)
- [x] Review apps/host-desktop
- [x] Verify and document findings into docs/bug-review.md
- [ ] Fix BUG-001 (session-replace ordering) — shut down previous session before starting the new one
- [ ] Fix BUG-002 (ble-helper execute kind-collapse) — handle tap/swipe/scroll plan kinds instead of falling back to the demo wiggle
- [ ] Fix BUG-003 (command deleted before execution + ack-write kills server)
- [ ] Fix BUG-004 (WinRT GATT handler blocking .join() inside the STA callback)
- [ ] Fix BUG-005 (HID command timeout vs. long macro -> orphaned ack files)
- [ ] Fix BUG-006 (Windows atomic preferences save data loss)
- [ ] Fix BUG-007 (control.ble run_for_output JoinHandle leak on timeout)
- [ ] Fix BUG-008 (session window close does not stop the runtime session)
- [ ] Fix BUG-009 (Recovering session cannot be stopped from the UI)
- [ ] Fix BUG-010 (process_exists trusts recycled PIDs)

This document is meant to be a living checklist. It reflects the current codebase, not the aspirational design docs under `docs/superpowers/`.

## Current Reality

- This repository is a Linux/Windows host workspace. There is no native iOS app target in this repo today.
- `apps/host-desktop` now starts, stops, and refreshes orchestrator-backed sessions locally.
- The capture plugins now require RGBA helper payloads, propagate frame metadata, and the host renders runtime-backed preview state from frame slots locally.
- The orchestrator now selects the actual control backend (`control.ble` when supported, otherwise `control.window-bridge`) and local/mock observed-change execution is reported as applied.
- Automated verification currently covers workspace, mock/plugin, contract, and docs-status flows. It does not cover a real iPhone/iPad end-to-end session.

## Remaining Work Checklist

### 1. Wire the desktop host to the real runtime

Plan: `docs/superpowers/plans/2026-04-04-host-runtime-and-operator-workflow.md`

- [ ] Decide whether persistent session/device state for reconnect and multi-device workflows is still in scope.
- [ ] If persistence is still in scope, implement save/restore for the selected device, selected capture source, and enough session metadata to support reconnect cleanly.
- [ ] If persistence is not in scope, narrow the reconnect/persistence claims in the top-level docs instead of leaving them implied.

### 2. Render a real live preview

Plan: `docs/superpowers/plans/2026-04-04-live-preview-capture-transport.md`

- [ ] Update `README.md` so the local mock/runtime-backed flow matches the current branch:
  - `host-desktop` is runtime-backed, not just a demo shell
  - the local mock E2E now reaches `SessionPhase::Streaming`
  - the local mock control plugin is currently `control.window-bridge`
  - the local mock execution result currently reports `applied == true` and `observed_change == true`
- [ ] Update `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md` so the Local mock row matches the current branch’s local mock control path and verified behavior.

### 3. Complete real control execution

Plan: `docs/superpowers/plans/2026-04-04-control-execution-and-observation.md`

- [ ] Add a telemetry/diagnostic breadcrumb when BLE startup/probe fails and the orchestrator falls back to `control.window-bridge`, so fallback does not silently hide BLE initialization failures.
- [ ] Validate the supported real-device control path on physical hardware instead of relying on local/mock observed-change semantics alone.

### 4. Close the host/operator workflow gaps

Plan: `docs/superpowers/plans/2026-04-04-host-runtime-and-operator-workflow.md`

- [ ] Re-check `README.md`, this file, and the acceptance matrix together after each remaining validation update so the top-level status docs stay aligned with the merged branch.

### 5. Add real-device validation

Plan: `docs/superpowers/plans/2026-04-04-real-device-validation-and-doc-alignment.md`

- [ ] Record each manual validation run with `docs/validation/real-device-session-template.md`.
- [ ] Run and record manual validation for:
  - Linux + window capture + BLE HID
  - Linux + window capture + window-input fallback
  - Windows + window capture + BLE HID
  - Windows + window capture + window-input fallback
  - any direct-receiver path that is intended to be supported
- [ ] Promote acceptance-matrix rows to `Verified` only after real operator validation.
- [ ] Add the strongest practical automated smoke coverage for real-runtime startup without depending on physical hardware in CI.

### 6. Keep status docs aligned with code

Plan: `docs/superpowers/plans/2026-04-04-real-device-validation-and-doc-alignment.md`

- [ ] Treat the design and plan docs in `docs/superpowers/` as historical planning artifacts unless the code and tests match them.
- [ ] Update the top-level docs immediately when a remaining checklist item moves from local/mock-only behavior to real verified behavior.

## Exit Criteria For "Complete App"

Do not describe this repository as a complete iOS remote-control app until all of the following are true:

- The desktop host starts and supervises real sessions through the orchestrator.
- The host renders a real live preview from a physical device or supported mirror path.
- Control actions are executed through a validated real-device path.
- Recovery and reconnect behavior are manually verified on supported platforms.
- The acceptance matrix records at least one fully verified real-device workflow.
