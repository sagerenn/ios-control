# iOS Screen Capture To PC Design

Date: 2026-04-02
Status: Design approved, awaiting written spec review

## Scope

This spec defines the host-side screen capture subsystem for getting the visible screen of a stock iPhone or iPad onto a Linux or Windows PC.

The agreed constraints are:

- Target devices are stock iPhone and iPad devices with no jailbreak.
- No companion iOS app is allowed.
- The solution must be software-only on the PC side.
- The iPhone or iPad and the PC may share the same local network.
- The primary V1 success criterion is human-usable remote viewing.
- Both capture paths are in scope from day one:
  - The project acts as a direct mirroring receiver.
  - The project ingests a third-party mirroring app window.
- Window ingestion may use any third-party mirroring app.
- Window ingestion is read-only in V1.

## Problem Statement

The overall project needs host-side access to iOS screen pixels so that a human operator or future planning layer can observe device state.

Under the agreed constraints, there are two viable product paths:

- Direct receiver: the project behaves as a local-network mirroring receiver selected by iOS.
- Window ingestion: the project captures the window contents of another app that is already receiving or displaying the iOS screen.

These are not the same subsystem internally. They have different risks, setup expectations, and error modes. This design therefore defines one shared host-side capture architecture with two backend families under a single normalized interface.

## Goals

- Provide one normalized frame stream API for the rest of the project.
- Support Linux and Windows from day one.
- Support both direct receiver and window ingestion source types.
- Optimize for human-usable remote viewing in V1.
- Keep backend-specific risks isolated behind replaceable adapters.
- Surface capture health explicitly instead of treating any open stream as good enough.

## Non-Goals

- Precise on-device coordinate mapping from the capture path alone.
- iOS-side automation or control through the capture subsystem.
- A companion iOS application.
- Extra dedicated capture hardware.
- App-specific guarantees about third-party mirroring software.

## Product Definition

The screen capture subsystem exposes a shared `VideoSource` interface to the rest of the project. That interface can be backed by either:

- `WindowIngestionBackend`
- `DirectReceiverBackend`

Everything above those backends consumes the same stream contract. Human viewing, recording, CV, and future planning do not depend on how the pixels were obtained.

This keeps the system composable with the Bluetooth control transport and prevents source-specific logic from leaking into future planner code.

## Architecture

The runtime architecture has four layers:

1. Capture Supervisor
2. VideoSource Interface
3. Backend Adapters
4. Frame Consumers

### 1. Capture Supervisor

This layer manages discovery, source selection, session lifecycle, retry policy, and normalized source identity.

Responsibilities:

- Discover available sources
- Select an active backend
- Start and stop capture sessions
- Remember previous successful sources
- Report session lifecycle state
- Normalize backend-specific availability into shared status

Example states:

- `Idle`
- `Discovering`
- `Ready`
- `Streaming`
- `Stalled`
- `Error`

### 2. VideoSource Interface

This is the stable cross-platform contract shared by both backends.

Core operations:

- `listSources()`
- `openSource(id)`
- `readFrame()`
- `getMetadata()`
- `close()`

Each emitted frame includes at least:

- Pixel buffer
- Width and height
- Orientation
- Timestamp
- Source id
- Source type
- Health flags such as `occluded`, `stalled`, or `resized`

### 3. Backend Adapters

The system has two backend families:

- `WindowIngestionBackend`
- `DirectReceiverBackend`

They are replaceable implementations of the same source contract, not separate applications.

### 4. Frame Consumers

The capture subsystem only publishes frames and health metadata. It does not decide what to do with the pixels.

Typical consumers:

- Human remote-view UI
- Recorder
- Future CV or planning pipeline

## Components

### WindowIngestionBackend

This backend captures pixels from an already-existing desktop window produced by another mirroring or screen-sharing application.

V1 rules:

- Read-only only
- No desktop automation
- No clicking into the source window
- No typing into the source window

Its responsibilities are:

- Enumerate candidate windows
- Open a selected window source
- Read frames from that window efficiently
- Detect window loss, resizing, and visibility problems
- Emit frames into the shared normalization pipeline

Platform guidance:

- On Windows, use native window capture APIs rather than screenshot polling.
- On Linux, prefer portal or compositor-native capture when available, with fallback to older window-system-specific paths only when necessary.

### DirectReceiverBackend

This backend makes the project itself available as a local-network mirroring target and accepts an incoming session from the iPhone or iPad.

This backend is explicitly higher risk than window ingestion. Stock iOS does not provide an Apple-supported receiver SDK for Linux or Windows, so this backend is compatibility work rather than a native platform integration.

Its responsibilities are:

- Advertise or expose the receiver as needed on the local network
- Accept an incoming mirroring session
- Perform backend-specific session setup and compatibility handshakes
- Decode or transform incoming media into the shared frame format
- Emit frames and health events into the shared normalization pipeline

### Frame Normalization Pipeline

Both backends feed a shared normalization stage.

Responsibilities:

- Convert frames into one internal pixel format
- Preserve timing metadata
- Track resize events
- Track orientation changes
- Detect stalled streams
- Mark unhealthy frame conditions consistently across backends

### Source Selection UX

The host application needs a clear selection model because both backend families can exist simultaneously.

Required capabilities:

- Pick a detected window source
- Wait for an incoming direct-receiver session
- Remember the last successful source
- Explain why a source is unavailable
- Prevent silent switching from one source to another

## Data Flow

There are two source pipelines that converge into one normalized frame stream.

### Window Ingestion Flow

1. The Capture Supervisor enumerates candidate desktop windows.
2. The user selects a source window.
3. The OS capture backend opens a read-only capture session for that window.
4. Frames are delivered into the normalization pipeline.
5. The normalizer emits `VideoFrame` objects with stable metadata.
6. If the source is minimized, occluded, resized, frozen, or closed, the backend emits a normalized health event.

### Direct Receiver Flow

1. The Capture Supervisor starts the direct receiver backend.
2. The receiver becomes available as a local-network mirroring target.
3. The iPhone or iPad selects that target from the system mirroring UI.
4. The backend accepts the incoming session and performs the required compatibility setup.
5. The backend decodes the incoming stream and feeds frames into the normalization pipeline.
6. The normalizer emits the same `VideoFrame` contract used by window ingestion.

### Shared Consumer Flow

Everything above the capture subsystem sees the same path:

`backend -> frame normalization -> VideoSource stream -> human viewer / recorder / future planner`

The rest of the project does not need backend-specific logic to consume pixels.

### Session Events

Both backends emit the same lifecycle events:

- `SourceDiscovered`
- `SourceReady`
- `StreamingStarted`
- `FrameDelivered`
- `StreamStalled`
- `StreamResized`
- `SourceLost`
- `StreamingStopped`
- `Error`

## Error Handling

Failure classes are normalized across both backends:

- `BackendUnavailable`
- `SourceNotFound`
- `StreamUnhealthy`
- `SourceAmbiguous`
- `CompatibilityFailure`

### BackendUnavailable

Examples:

- Required OS capture APIs are unavailable
- Required permissions are missing
- Direct receiver local services cannot start

Response:

- Fail early
- Surface the backend-specific cause
- Avoid pretending the source can stream when startup failed

### SourceNotFound

Examples:

- The selected window disappeared
- The selected source changed identity
- The expected direct receiver session never arrives

Response:

- Mark the source unavailable
- Keep prior source identity visible to the user
- Require an explicit new selection if source identity changed

### StreamUnhealthy

Examples:

- Frames stop arriving
- Timestamps stall
- Window capture turns black
- Decode fails
- Orientation or size changes unexpectedly

Response:

- Emit health flags rather than silently serving stale frames
- Distinguish `stream open` from `stream usable`

### SourceAmbiguous

Examples:

- Multiple candidate windows appear valid
- The system cannot safely auto-select one source

Response:

- Do not silently switch between candidate sources
- Require explicit user confirmation

### CompatibilityFailure

Examples:

- The direct receiver backend cannot successfully interoperate with the iOS mirroring session on the current OS, network, or build

Response:

- Surface backend-specific detail in logs
- Keep the shared API failure category normalized
- Preserve window ingestion as an independent source path

## Logging And Diagnostics

The system should log:

- Backend startup results
- Source discovery results
- Source identity changes
- Session lifecycle transitions
- Frame delivery timing
- Resize and orientation events
- Stream stall detection
- Backend-specific failure details

The logs must make it possible to distinguish:

- The backend never started
- The source existed but no usable frames arrived
- Frames arrived but health degraded

## Testing Strategy

### Unit Tests

Cover shared behavior:

- Frame metadata normalization
- Resize handling
- Orientation handling
- Stall detection
- Source identity tracking
- Event ordering

### Backend Contract Tests

Both backend families must satisfy a shared contract:

- Discover
- Open
- Deliver frames
- Report health
- Stop
- Recover or fail cleanly

### Platform Tests

Required platform coverage:

- Windows window capture against a normal resizable app window
- Linux capture through the preferred portal or compositor path
- Window loss when a captured window closes
- Health degradation when a source is minimized or stalls
- Direct receiver startup on Linux
- Direct receiver startup on Windows
- Direct receiver session acceptance from iPhone
- Direct receiver session acceptance from iPad

### Acceptance Criteria

V1 is successful when:

- The user can choose a source on Linux and Windows.
- The system can deliver a live frame stream suitable for human remote viewing.
- The system can report when a stream is present but not trustworthy.
- Both backend families feed the same `VideoSource` API.
- The rest of the project can consume capture frames without source-specific logic.

## Risks

- Direct receiver support on Linux and Windows depends on protocol compatibility rather than an Apple-supported PC receiver path.
- Third-party mirroring apps may change window titles, rendering behavior, or output characteristics.
- Linux capture behavior varies by compositor and portal availability.
- A technically open stream may still be unusable for planning if frames are stale, occluded, or low quality.

## Relationship To Bluetooth Control

This subsystem is intentionally separate from the Bluetooth control transport.

Capture provides pixels and health metadata.
Bluetooth control provides input delivery to iOS.

Future planner layers may sit above both subsystems:

- Consume frames from capture
- Infer target UI state
- Emit abstract actions into the Bluetooth transport

This separation keeps the design modular and avoids embedding control assumptions into the capture layer.

## References

- Apple Support: Use AirPlay to stream video or mirror the screen of your iPhone or iPad
  https://support.apple.com/en-us/102661
- Apple Support: AirPlay to Mac system requirements
  https://support.apple.com/en-afri/108046
- Microsoft Learn: Screen capture
  https://learn.microsoft.com/en-us/windows/uwp/audio-video-camera/screen-capture
- XDG Desktop Portal: ScreenCast
  https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html
