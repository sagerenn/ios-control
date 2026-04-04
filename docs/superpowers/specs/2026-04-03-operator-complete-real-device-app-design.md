# Operator-Complete Real-Device App Design

> Historical planning artifact. This design describes an intended target state from 2026-04-03. It does not reflect the current branch reality on its own. For current status, use `README.md`, `docs/TODO.md`, and `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`.

**Date:** 2026-04-03

## Goal

Turn the current mock-oriented desktop shell into an operator-grade, cross-platform control console that can manage multiple concurrent physical iPhone/iPad sessions, using real capture and real control backends where available, while exposing explicit fallback, diagnostics, and recovery behavior.

## Scope

This design targets an operator-complete product, not a polished end-user setup flow.

In scope:

- Linux and Windows as equal first-class host targets
- Multiple concurrent device sessions from one host
- Real device capture through accepted runtime backends
- BLE HID as the preferred control path
- Explicit fallback control when BLE peripheral mode is unavailable
- Capability discovery, session supervision, diagnostics, and bounded recovery
- Operator-facing setup and validation documentation

Out of scope:

- Consumer-grade installer or onboarding flow
- Zero-dependency runtime support
- Claiming that CI alone validates real hardware behavior

## Constraints

- External runtime helpers are allowed.
- Linux and Windows must share the same product contract even when their internals differ.
- BLE is preferred, but the app must degrade to a visible fallback path rather than fail closed when BLE peripheral support is missing.
- The first complete milestone must support full multi-device operation rather than one active device at a time.

## Recommended Architecture

Use one host app with one shared session model and pluggable capture and control backends.

The host owns:

- capability discovery
- device inventory
- session lifecycle
- multi-device supervision
- diagnostics and telemetry
- fallback policy
- operator workflow and UI

Backends own:

- OS-specific capture implementation
- helper-specific capture implementation
- BLE transport implementation
- fallback control implementation

Grounding and execution stay in the shared runtime surface so that planning, execution reporting, and recovery semantics remain stable across host platforms and backend choices.

This architecture is preferred over platform-specific vertical slices because it keeps Linux and Windows behavior aligned, reduces duplication, and creates clean seams for parallel implementation.

## System Model

The system is organized around a `device session`.

Each device session contains three functional planes:

1. `capture`
   Produces a live frame stream and capture health.
2. `control`
   Exposes control readiness and executes device actions.
3. `grounding/execution`
   Converts intent into executable actions and evaluates outcomes.

Each session is supervised independently. A failure in one device session must not destabilize other active sessions.

Shared host state is limited to:

- discovered device inventory
- backend capability inventory
- adapter allocation limits
- UI coordination
- global telemetry aggregation

Everything else is per-session state.

## Capability Model

The host must probe and store explicit capabilities rather than assume a host can run every path.

### Capture capabilities

- mirrored window capture availability
- direct receiver helper discovery
- helper executable health
- source enumeration availability
- expected frame cadence and slot sizing support

### Control capabilities

- BLE peripheral support
- pairing readiness
- mirrored-window input fallback availability
- transport-specific limitations

### Session-scale capabilities

- concurrent helper/session limits
- per-backend adapter allocation
- per-host resource exhaustion conditions

### Operator prerequisites

- required external helper or mirroring software
- setup steps that must be completed before a backend is usable

Capability discovery must surface both availability and reasoned failure so the host can explain why a path is unavailable.

## Session Composition

Session creation must be deterministic and capability-driven.

For each device session:

1. Select a physical device record.
2. Select a capture backend for that device.
3. Select a control backend, preferring BLE.
4. If BLE is unavailable or fails, degrade to an explicit fallback control backend.
5. Attach grounding and execution.
6. Start long-lived session supervision.
7. Continuously publish session health, capture health, control readiness, backend identity, and recovery state.

The host must never silently switch backends. Any change in capture or control path is operator-visible.

## Capture Design

Capture becomes a backend family behind one live stream contract.

Planned backend categories:

- mirrored-window capture
- direct receiver/helper capture
- future native capture paths if added later

All capture backends must present the same host-facing surface:

- source identity
- stream open/close lifecycle
- frame metadata
- frame health
- slot transport details
- backend-specific diagnostics

Capture backends are allowed to depend on external helpers, but helper dependence must be explicit in capability reporting and operator setup guidance.

## Control Design

Control becomes a backend family with a preferred path and a fallback path.

Preferred path:

- BLE HID transport when the host supports peripheral mode and pairing succeeds

Fallback path:

- desktop input injection against the active mirrored/receiver window when the chosen capture helper supports host keyboard or pointer forwarding

Control backends must expose:

- capability status
- setup checklist
- live control phase
- execution summary
- transport-specific failure reasons

The host policy is:

- prefer BLE whenever available
- fall back only after explicit capability failure or bounded runtime failure
- surface the exact reason for fallback
- allow operator rebind/retry actions per device

## Runtime And Orchestrator Design

The current one-shot orchestrator model must become a long-lived supervisor over session actors.

Each session actor owns:

- plugin/helper processes
- frame transport handles
- control transport state
- recovery timers and retry counters
- per-device telemetry
- current backend selection

The supervisor owns:

- device/session registry
- actor startup and shutdown
- concurrent session coordination
- resource arbitration
- fan-in of diagnostics to the host UI

This is the key structural gap between the current mock E2E path and a complete app.

## Session State Model

Each session must expose explicit operator-visible sub-states:

- `discovering`
- `starting_capture`
- `streaming`
- `starting_control`
- `control_ready`
- `degraded_capture`
- `degraded_control`
- `recovering`
- `operator_action_required`
- `stopped`

These states are more useful than one coarse phase because they let the operator understand whether the session is healthy, degraded, retrying, or blocked on manual intervention.

## Recovery Policy

Recovery must be bounded, visible, and per-session.

Rules:

- retry transient helper or transport failures automatically
- preserve session identity and telemetry across retries
- degrade from BLE to fallback control only after a clear threshold
- never silently change capture or control backend
- escalate to `operator_action_required` when automated recovery is exhausted

Typical surfaced failure classes:

- helper missing
- helper exited unexpectedly
- pairing failure
- stale or missing mirrored window
- lost frame cadence
- frame slot mismatch
- per-device resource exhaustion

Every surfaced failure must include remediation text that the operator can act on.

## Host UI Design

The desktop host becomes an operator console with three layers.

### Fleet dashboard

Shows:

- all discovered or known devices
- current session phase
- selected capture backend
- selected control backend
- health summary
- outstanding operator actions

### Session workspace

One workspace per active device showing:

- live preview metadata
- capture and control readiness
- last execution result
- recovery state
- session actions such as start, stop, retry, rebind backend, and acknowledge checklist items

### Diagnostics and setup surface

Shows:

- host capability report
- per-device capability report
- helper discovery and health
- BLE support and pairing status
- frame cadence and helper stderr/exit history
- operator remediation guidance

For full multi-device operation, the first complete UI should stay in one app window with a dashboard and selectable or tabbed device workspaces. That supports concurrency without multiplying top-level windows.

## Operator Workflow

The intended workflow is:

1. Launch the host.
2. Review host capabilities and discovered devices.
3. Start or resume sessions for selected devices.
4. Observe live health, active backends, and diagnostics.
5. Intervene only when a session enters `operator_action_required`.

This is intentionally operator-oriented rather than consumer-oriented.

## Testing Strategy

The testing strategy must match the real product boundary.

### Automated tests

- contract tests for runtime messages and state transitions
- backend tests for capability probes and helper discovery
- orchestrator tests for multi-device supervision, degradation, fallback, and recovery
- host view-model tests for concurrent operator workflows

### Operator-run validation

Because CI cannot prove physical BLE or mirroring behavior, maintain an operator validation matrix for:

- Linux + real device flows
- Windows + real device flows
- BLE-preferred sessions
- fallback-control sessions
- multi-device concurrency
- recovery after helper/control failures

## Definition Of Complete For This Milestone

The milestone is complete when:

- multiple physical devices can be tracked concurrently
- each device can start a real capture session through an accepted backend
- each device can establish BLE control when supported, or degrade to a visible fallback control path
- the fallback control path is the mirrored-window input bridge, not a mock or operator-only placeholder
- the host surfaces live health, diagnostics, and recovery actions
- failure and fallback are explicit and bounded
- operator documentation reflects actual dependencies, workflow, and limits

## Risks And Trade-Offs

- Cross-platform parity increases interface work up front, but avoids long-term drift.
- External helpers speed delivery, but require stronger diagnostics and setup UX.
- Full multi-device support increases supervision complexity, but matches the chosen success bar and avoids redesigning the runtime later.
- BLE fallback improves practical usability, but must stay explicit to avoid misleading operators about the active control path.

## Recommended Execution Order

Implementation should proceed in parallel across these sub-projects:

1. real capture backends
2. real BLE and fallback control backends
3. long-lived session/runtime supervision
4. host UI and operator workflow

These streams should share one stable contract layer and one session model from the start.
