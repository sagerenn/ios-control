# Cross-Platform Host App For Linux And Windows Design

Date: 2026-04-02
Status: Design approved, awaiting written spec review

## Scope

This spec defines the cross-platform host application that runs on Linux and Windows and coordinates the previously designed subsystems:

- Bluetooth control
- Screen capture
- Coordinate mapping and action grounding

The agreed constraints are:

- V1 is a desktop app with a full UI.
- The app must serve both normal end users and developer or research operators.
- The app should provide guided defaults with advanced configuration panels.
- The runtime architecture is plugin-oriented.
- Multiple devices may be captured and controlled concurrently.

## Problem Statement

The overall project needs a host-side product surface that can:

- Present one coherent user experience
- Manage multiple device sessions
- Route work to the appropriate backends
- Surface host capability limits clearly
- Keep risky subsystems isolated from the desktop shell

Without a dedicated host app architecture, the system would collapse into backend-specific controls, duplicated setup logic, and poor failure containment.

## Goals

- Provide one cross-platform desktop control center for Linux and Windows.
- Support guided setup for end users and advanced inspection for operators.
- Coordinate concurrent multi-device sessions.
- Isolate risky subsystem behavior behind plugin contracts.
- Persist device, host, and session state cleanly.
- Surface actionable diagnostics without exposing backend internals by default.

## Non-Goals

- A headless-first service architecture for V1.
- A public third-party plugin marketplace.
- Arbitrary untrusted extension loading.
- Embedding backend-specific business logic directly in the desktop shell.

## Product Definition

The host app is a desktop control center with a plugin-oriented runtime.

The desktop shell owns:

- Windows and navigation
- Setup flows
- Session dashboards
- Per-device views
- Advanced settings
- Diagnostics presentation

The runtime below the shell owns:

- Session orchestration
- Capability discovery
- Plugin supervision
- Device registry
- State and telemetry persistence

Backend implementations for capture, control, and grounding are treated as local plugins behind stable contracts.

## Architecture

The host app has five major layers:

1. Desktop Shell
2. Session Orchestrator
3. Capability Registry
4. Plugin Runtime
5. State And Telemetry Store

### 1. Desktop Shell

This is the visible Linux and Windows application.

Responsibilities:

- Window management
- Navigation
- Device dashboards
- Live preview panes
- Setup wizards
- Per-device control panels
- Advanced settings
- Diagnostics and logs

The shell presents both guided flows and operator-facing visibility without becoming two separate applications.

### 2. Session Orchestrator

This is the central coordination layer for multi-device runtime behavior.

It manages:

- Active sessions
- Session lifecycle transitions
- Backend assignment
- Concurrency limits
- Session-local state
- Cross-plugin event routing

Each device session is treated as its own session graph with distinct:

- Capture source
- Control transport
- Grounding state
- Diagnostics stream

### 3. Capability Registry

This layer records what the current host machine can actually support.

Examples:

- Windows BLE peripheral support
- Linux Bluetooth backend viability
- Capture backend availability
- Plugin health and version state

It exists so setup and routing decisions are based on measured capability rather than static assumptions.

### 4. Plugin Runtime

This layer loads and supervises local backend plugins.

Initial plugin families:

- `capture.*`
- `control.*`
- `grounding.*`

Future families may include:

- `scene.*`
- `planning.*`

The shell is not allowed to contain backend-specific implementation logic. It communicates through plugin contracts, not direct backend assumptions.

### 5. State And Telemetry Store

This layer persists:

- App configuration
- Plugin configuration
- Known devices
- Capability snapshots
- Session history
- Logs and diagnostics

## Components

### Desktop Shell

The main app process should provide:

- Device list
- Multi-session dashboard
- Live preview panes
- Setup wizards
- Per-device control panels
- Advanced settings
- Diagnostics viewer

It must support two user modes in one product:

- Guided default usage
- Operator inspection and debugging

### Session Orchestrator

This component creates and manages one session per active device.

Each session binds together:

- A capture plugin instance
- A control plugin instance
- An optional grounding plugin instance
- Session-local telemetry and state

It owns:

- Start, pause, resume, reconnect, and teardown
- Concurrency policy
- Resource limits
- Failure containment boundaries

### Plugin Runtime

This component discovers, loads, starts, monitors, and restarts plugins.

Each plugin should declare:

- Plugin id
- Version
- Supported OSes
- Capabilities
- Required prerequisites or permissions
- Health state
- Config schema

Plugins are local runtime backends, not remote extensions.

### Capability Registry

This component runs and caches capability probes.

Example questions it answers:

- Can this Windows adapter support the required Bluetooth role?
- Is the current Linux capture path available on this compositor?
- Which direct receiver path is available on this host?
- Which installed plugins are healthy and usable?

### Device Registry

This component tracks:

- Known iPhones and iPads
- Pair history
- Preferred plugins
- Last successful session state
- Device-specific quirks

### State And Telemetry Store

This component stores:

- App configuration
- Plugin configuration
- Known devices
- Session history
- Structured logs
- Crash and failure diagnostics

### UI Modules

The UI should be split by concern rather than by backend:

- `Onboarding`
- `Dashboard`
- `Device Detail`
- `Session View`
- `Advanced Settings`
- `Diagnostics`

### Contract Layer

This is the internal API boundary between the shell or orchestrator and plugins.

It normalizes:

- Session states
- Frame streams
- Action streams
- Health events
- Error categories
- Capability reports

This layer is what keeps the plugin model maintainable.

## Data Flow

The host app turns host capabilities and user intent into running per-device session graphs.

### 1. Startup

1. Desktop Shell starts.
2. Plugin Runtime discovers installed plugins.
3. Capability Registry runs host probes and caches results.
4. Device Registry loads known devices and prior preferences.
5. The UI shows what this machine can and cannot support.

### 2. Onboarding And Setup

For a new device, the user follows a guided flow:

1. Choose or confirm device
2. Complete required control setup
3. Choose a capture source or backend
4. Validate host capabilities
5. Store preferred session configuration

Advanced users may override backend choices, but the default path should be capability-guided.

### 3. Session Creation

When a session starts, the Session Orchestrator creates a session graph consisting of:

- Selected capture plugin instance
- Selected control plugin instance
- Optional grounding plugin instance
- Session-local telemetry stream

Multiple sessions may run concurrently for multiple devices.

### 4. Runtime Data Paths

Within a live session, the normalized paths are:

- `capture plugin -> frame stream -> session view / future planner`
- `UI or grounding plugin -> abstract HID actions -> control plugin`
- `all plugins -> health/error events -> orchestrator -> UI/telemetry store`

The desktop shell does not speak backend-specific protocols directly.

### 5. State Updates

During runtime:

- Plugin health updates feed the orchestrator
- Successful routing choices and quirks may be persisted
- Logs and diagnostics are stored continuously
- The UI reflects per-device state in near real time

### 6. Failure Handling

If one plugin degrades or fails:

- The orchestrator marks the affected session edge or plugin unhealthy
- Other sessions remain alive where possible
- The UI surfaces actionable failure detail and fallback guidance

### 7. Teardown And Resume

When a session ends:

- Plugin instances are closed cleanly
- Useful session metadata is persisted
- Enough state remains to restore the session later without full reconfiguration

## Error Handling

The host app's primary responsibility in failure scenarios is containment.

Failure classes for V1:

- `PluginUnavailable`
- `CapabilityMismatch`
- `SessionPartialFailure`
- `StateDrift`
- `ConcurrencyPressure`

### PluginUnavailable

A required plugin is missing, disabled, crashed, or unsupported on the current OS.

Response:

- Fail only the affected path
- Surface actionable UI-level messaging
- Preserve detailed diagnostics for operators

### CapabilityMismatch

The current host machine, adapter, compositor, or network environment cannot support the chosen session path.

Response:

- Report capability evidence clearly
- Suggest viable alternatives when available
- Avoid pretending a path is supported

### SessionPartialFailure

One session edge fails, such as capture degrading while control remains connected.

Response:

- Isolate the failure to the affected session component
- Keep unrelated sessions alive
- Preserve remaining healthy components where useful

### StateDrift

Persisted preferences no longer match the current host reality.

Response:

- Invalidate stale selections cleanly
- Re-run capability-guided routing
- Explain why the old preference is no longer valid

### ConcurrencyPressure

Multiple simultaneous sessions exceed practical host limits.

Response:

- Surface resource pressure
- Throttle or limit new session starts when needed
- Prefer preserving existing healthy sessions

## Logging And Diagnostics

The host app should record:

- Capability probe results
- Plugin discovery and health transitions
- Session lifecycle transitions
- Routing decisions
- Structured errors
- Resource pressure indicators
- Crash or restart events

Diagnostics should support both audiences:

- concise guided explanations for end users
- detailed structured context for operators

## Testing Strategy

### Unit Tests

Cover:

- Orchestrator state transitions
- Capability resolution
- Plugin contract normalization
- Preference routing

### Plugin Contract Tests

Verify that every plugin family reports:

- Capabilities
- Health
- Config schema
- Errors
- Lifecycle transitions

in the same normalized shape.

### Integration Tests

Verify:

- Multi-device session creation
- Plugin crash and restart handling
- Partial session failure containment
- Concurrent session monitoring

### UI Acceptance Tests

Verify:

- Onboarding
- Dashboard behavior
- Advanced settings visibility
- Diagnostics navigation
- Concurrent session state visibility

### Acceptance Criteria

V1 is successful when:

- The app launches on Linux and Windows.
- It accurately reports host capabilities.
- Users can configure and run concurrent device sessions from one desktop UI.
- Plugin and backend failures are isolated and diagnosable.
- Guided defaults work while advanced users can inspect and override backend choices.
- The shell remains stable when experimental backends fail.

## Risks

- Plugin-oriented runtime boundaries can become ad hoc unless the contract layer stays strict.
- Supporting both end-user and operator workflows in one UI can create clutter if not separated carefully.
- Concurrent sessions increase CPU, memory, decode, and Bluetooth pressure quickly.
- Experimental backends may behave differently across Linux and Windows and stress the shell in inconsistent ways.

## Relationship To Other Specs

This host app coordinates the previously designed subsystems:

- Bluetooth control transport
- Screen capture
- Coordinate mapping and action grounding

It does not redefine those subsystem contracts. It composes them into one user-facing product.

## References

- Internal project specs written on 2026-04-02 for:
  - Bluetooth control path to iOS
  - iOS screen capture to PC
  - Coordinate mapping and action grounding
