# Plugin Capture Direct UxPlay Integration Design

**Goal:** Replace the mock direct-receiver behavior in `plugin-capture-direct` with a real AirPlay receiver path that starts from the host app on double-click, makes the PC discoverable to iPhone/iPad Screen Mirroring, delivers live video frames into the existing capture pipeline, and ingests/routes AirPlay audio on both Linux and Windows.

## Scope

This design covers the direct receiver implementation boundary, runtime packaging, plugin/session integration, status reporting, and validation strategy needed to back `plugin-capture-direct` with UxPlay.

In scope:
- bundling and supervising a real AirPlay-compatible receiver runtime
- using UxPlay as the bundled receiver engine
- Linux and Windows release/runtime support
- video ingestion into the existing frame-slot-based capture path
- AirPlay audio ingestion and local playback routing
- host/plugin/orchestrator changes needed to expose real readiness and real session status
- direct receiver startup from the existing launcher double-click flow
- deterministic automated tests plus a manual validation matrix

Out of scope:
- macOS support
- building a Rust-native AirPlay receiver protocol stack from scratch
- inventing a new direct receiver UI flow separate from the current launcher/session window
- recording/exporting AirPlay audio in v1
- multi-session direct receiver support in one host process
- full DNS-SD/Bonjour runtime support in v1 when BLE discovery is sufficient

## Current Problem

Today the host app does start a `CaptureBackend::Direct` session when the launcher double-click path chooses the direct backend, but that path is not a real AirPlay server.

Current limitations:
- `plugin-capture-direct` currently self-probes and self-streams using its own binary as a fake helper
- startup readiness marks the direct receiver as available even when no real network-advertised receiver exists
- there is no Bonjour, BLE advertisement, RAOP, or AirPlay session stack in this repository
- the iPhone Screen Mirroring picker cannot discover the PC because no real receiver is advertised
- audio is not modeled as a first-class part of the direct receiver path

This creates a misleading UX: the launcher suggests the PC can act as a mirroring target, but the runtime only produces mock frames.

## Recommended Approach

Use **UxPlay as a bundled sidecar runtime** and make `plugin-capture-direct` the supervisor and normalization layer around it.

Recommended implementation boundary:
- `UxPlay`
  owns AirPlay discovery, pairing, session acceptance, protocol handling, and source media emission
- `plugin-capture-direct`
  owns runtime discovery/probing, child-process supervision, RTP receive/decode plumbing, frame-slot writes, audio playback routing, and status reporting back into this repo

This is preferred over capturing a UxPlay window because window capture would add unnecessary latency and fragility and would collapse the direct receiver path back into a helper-window ingestion pattern.

This is preferred over embedding a forked UxPlay library because upstream UxPlay is published as an application/runtime, not as a stable embeddable SDK. The sidecar boundary gets a production-capable result sooner while preserving the option for a separate follow-on design to deepen the integration.

## Receiver Architecture

`plugin-capture-direct` should stop pretending to be the AirPlay server itself. It should become a runtime supervisor around a bundled UxPlay deployment plus repo-owned media adapters.

Process layout:
- `host-desktop`
  launches `plugin-capture-direct` through the existing plugin runtime/orchestrator path
- `plugin-capture-direct`
  allocates local ports, frame slots, audio sinks, and status state, then launches and supervises bundled `uxplay`
- `uxplay`
  advertises the receiver, accepts the iPhone/iPad connection, and emits media to repo-owned local transport endpoints
- bundled beacon helper
  advertises the UxPlay-generated BLE discovery payload without requiring Python or operator-managed tooling

Media flow:
- Video:
  - `plugin-capture-direct` reserves a localhost video port
  - UxPlay is launched with `-vrtp <pipeline>` targeting a localhost RTP sender
  - the plugin runs a local RTP video receive/decode path
  - decoded frames are converted to RGBA
  - RGBA bytes are written into the existing shared-memory frame slot
  - `ReadCaptureFrame` returns real frame metadata from that slot
- Audio:
  - `plugin-capture-direct` reserves a localhost audio port
  - UxPlay is launched with `-artp <pipeline>` targeting a localhost RTP sender
  - the plugin runs a local RTP audio receive/decode path
  - decoded audio is normalized into a stable playback format inside the plugin
  - the plugin routes audio to local host playback and updates audio status/diagnostics

Session lifecycle:
1. Operator double-clicks a startable Bluetooth device row.
2. `host-desktop` starts a direct capture session for that device.
3. `plugin-capture-direct` resolves the bundled UxPlay runtime and beacon helper.
4. The plugin reserves localhost media ports and creates a session-specific BLE discovery file path.
5. The plugin launches UxPlay and the beacon helper.
6. The iPhone/iPad discovers the PC through the advertised direct receiver path.
7. UxPlay accepts the AirPlay session and starts sending forwarded video/audio to localhost RTP receivers owned by the plugin.
8. The plugin publishes real video frames and audio status.
9. The host session window transitions from waiting to streaming when the first frame arrives.
10. Stopping the session tears down UxPlay, beacon helper, RTP receivers, and local media resources.

## Packaging And Runtime Layout

The direct receiver must be self-contained in release artifacts. Operators should not need to install UxPlay, Bonjour, Python, or GStreamer separately.

Recommended bundle layout:

- `plugins/plugin-capture-direct(.exe)`
- `runtime/uxplay/<platform>/uxplay(.exe)`
- `runtime/uxplay/<platform>/gstreamer/...`
- `runtime/uxplay/<platform>/beacon-helper(.exe)`
- `runtime/uxplay/<platform>/manifest.json`

The runtime manifest should record:
- UxPlay version
- packaged GStreamer runtime version
- relative executable/library/plugin paths
- supported discovery mode(s)
- required runtime env vars

Linux runtime rules:
- package `uxplay` and a private GStreamer runtime tree
- set `LD_LIBRARY_PATH` and `GST_PLUGIN_PATH` for the UxPlay child process
- package only the needed plugin set for RTP, depayload, decode, convert, resample, and host playback

Windows runtime rules:
- build/package `uxplay.exe` using the upstream-supported MSYS2/UCRT64 approach
- stage required DLLs and required GStreamer plugin DLLs beside the runtime tree
- set `PATH` and `GST_PLUGIN_PATH_1_0` for child processes
- treat Bonjour SDK as a build dependency only, not an operator runtime requirement

## Discovery Strategy

Use **BLE discovery as the default v1 discovery mechanism on both Linux and Windows**.

Reasoning:
- it avoids requiring a separately installed Bonjour runtime on Windows
- it keeps Linux and Windows aligned around one self-contained discovery model
- UxPlay already has a BLE-oriented discovery path, but upstream expects a Python helper
- this repository can replace that Python dependency with a small bundled Rust beacon helper

Beacon-helper design:
- UxPlay writes a session-specific BLE discovery file
- bundled `direct-beacon` watches/reads that file
- Linux implementation advertises through BlueZ/DBus
- Windows implementation advertises through WinRT BLE APIs
- the helper is launched and owned by `plugin-capture-direct`

Future extension:
- optional DNS-SD/Bonjour compatibility mode can be added in a follow-on design for networks or client versions where BLE discovery is insufficient
- v1 does not need to block on that expansion

## Readiness Model

The direct receiver must no longer probe as “ready” just because `plugin-capture-direct` can execute.

Direct-receiver readiness should require all of the following:
- bundled `uxplay` binary exists
- bundled runtime manifest exists and is readable
- required GStreamer runtime files are present
- bundled beacon helper exists
- localhost RTP ports can be reserved
- at least one supported discovery backend is available on the current host
- UxPlay can be launched for a lightweight runtime probe without immediate dependency failure

If any prerequisite fails:
- startup marks direct receiver as blocked or error with the concrete reason
- launcher rows that rely on direct receiver startup are not startable
- the host app stays otherwise usable for non-direct flows

## Repo Structure Changes

The UxPlay-backed implementation should keep the mock path for tests while adding a real runtime path for packaged/manual use.

Recommended changes in `plugins/capture-direct`:
- keep `src/mock_backend.rs` for deterministic local tests
- add `src/runtime_bundle.rs`
  - resolve bundled runtime files and manifest from workspace or bundle layout
- add `src/uxplay_launcher.rs`
  - reserve ports, build args/env, launch and supervise UxPlay
- add `src/rtp_video.rs`
  - own the localhost RTP video receive/decode/normalize path
- add `src/rtp_audio.rs`
  - own the localhost RTP audio receive/decode/playback/status path
- add `src/direct_status.rs`
  - own direct session state and operator-facing status values
- refactor `src/main.rs`
  - choose between mock mode and bundled UxPlay mode
  - remove the current “self as helper” assumption from real runtime mode

New bundled helper:
- add `helpers/direct-beacon`
  - Linux BLE advertising backend
  - Windows BLE advertising backend

Host/bootstrap/runtime changes:
- `apps/host-desktop/src/bootstrap/runtime_locator.rs`
  - resolve the UxPlay runtime tree and beacon helper
- `apps/host-desktop/src/bootstrap/model.rs`
  - carry helper/runtime paths for the direct runtime bundle
- `apps/host-desktop/src/bootstrap/capability_probe.rs`
  - probe actual direct runtime readiness instead of plugin self-probe
- `apps/host-desktop/src/runtime.rs`
  - carry capture/audio status from the active direct session
- `apps/host-desktop/src/app.rs`
  - expose clearer waiting/degraded/error states for the direct path
- `apps/host-desktop/src/panels/session_view.rs`
  - render direct-session audio and degraded-state information

Packaging/release changes:
- `scripts/package_release.py`
  - stage the bundled UxPlay runtime tree and beacon helper
- add build/packaging support scripts for:
  - vendored/built UxPlay outputs
  - pruned GStreamer runtime staging
  - direct-runtime manifest generation

## Contract Changes

The first version should keep raw audio playback inside `plugin-capture-direct` instead of inventing a host-level PCM transport. The rest of the application should receive **status metadata**, not audio buffers.

Recommended contract additions:

### Capture Contracts

Extend `crates/contracts/src/capture.rs` with:
- `AudioStreamPhase`
- `AudioRoute`
- `AudioStreamStatus`
- `CaptureStatus`

`CaptureStatus` should include at least:
- video session phase
- latest frame health
- audio phase
- audio route
- whether audio playback is active
- direct-receiver-specific degraded reason when present

### Plugin Protocol

Extend `crates/plugin-protocol/src/lib.rs` with:
- `HostToPlugin::GetCaptureStatus`
- `PluginToHost::CaptureStatus { status }`

This lets the orchestrator and host query a live direct session without overloading `ReadCaptureFrame`.

### Orchestrator / Host Runtime

Extend:
- `crates/session-orchestrator/src/lib.rs`
- `apps/host-desktop/src/runtime.rs`

So active sessions can surface capture/audio status into runtime workspace state and diagnostics.

The v1 rule is:
- video frames still flow through the existing capture frame slot
- audio stays in the plugin for playback
- host UI/orchestrator consume only audio/capture status metadata

This keeps the first real AirPlay integration tractable while still fulfilling the requirement to ingest and route AirPlay audio.

## Error Handling

The direct receiver should distinguish startup failures from in-session failures and always surface concrete operator-facing reasons.

Failure classes:
- `RuntimeMissing`
  - bundled UxPlay, GStreamer files, manifest, or beacon helper missing
- `RuntimeLaunchFailed`
  - child process cannot start or exits immediately
- `DiscoveryUnavailable`
  - beacon helper cannot advertise or host BLE support is unavailable
- `SessionNotConnected`
  - receiver is advertising but no iPhone/iPad has connected yet
- `VideoPipelineFailed`
  - RTP video receive/decode path failed
- `AudioPipelineFailed`
  - RTP audio receive/decode/playback path failed
- `StreamStalled`
  - no new media packets arrive within timeout after a session had started
- `SessionEnded`
  - remote device stopped mirroring cleanly

Operator-facing behavior:
- launcher startability is blocked only by startup/runtime readiness failures
- active sessions can show:
  - `Waiting for iPhone connection`
  - `Connected, waiting for first frame`
  - `Streaming`
  - `Streaming with audio degraded`
  - `Receiver error: <reason>`
- audio failure degrades but does not immediately kill the session if video continues
- video failure ends the direct capture session and surfaces a restartable error

Diagnostics/logging should capture:
- UxPlay launch args
- env vars and runtime roots used
- selected video/audio ports
- beacon helper launch/exit status
- UxPlay exit status
- last RTP packet timestamps
- last meaningful UxPlay/GStreamer stderr lines

## Testing Strategy

Because CI cannot prove real iPhone AirPlay behavior, automated tests should focus on deterministic boundaries and state transitions.

Automated coverage:

### `plugin-capture-direct`

- runtime bundle resolution tests
- UxPlay launch-argument construction tests
- Linux/Windows child env construction tests
- failure mapping tests from runtime/probe/port/process errors into `CaptureStatus`
- protocol tests:
  - `ProbeCapture` fails when runtime bundle is incomplete
  - `OpenCaptureStream` starts direct runtime supervision
  - `ReadCaptureFrame` transitions from waiting to streaming when frames arrive
  - `GetCaptureStatus` reports audio-active/audio-degraded states

### Orchestrator

- direct sessions stay in connecting state before first frame
- direct sessions transition to streaming when first frame arrives
- degraded audio does not immediately terminate a video-capable direct session

### Host App

- launcher startability depends on actual direct-runtime readiness
- direct session window shows waiting/streaming/degraded/error states correctly

### Packaging

- Linux release bundle contains packaged UxPlay runtime tree and beacon helper
- Windows release bundle contains packaged UxPlay runtime tree and beacon helper
- runtime manifest paths in staged artifacts are internally valid

Manual validation matrix:
- Linux:
  - double-click makes the PC appear in iPhone Screen Mirroring
  - connection produces live preview
  - audio plays locally on the host
  - stopping the session removes discoverability
- Windows:
  - same validation goals
  - verify no external Bonjour runtime installation is required
- recovery:
  - stop mirroring from iPhone
  - start mirroring again
  - kill beacon helper
  - kill UxPlay child
  - temporarily disrupt network connectivity

## Risks And Tradeoffs

Primary risks:
- packaging a private cross-platform GStreamer runtime reliably
- BLE discovery behavior differing across host hardware/OS versions
- UxPlay CLI/runtime assumptions changing across upstream releases
- RTP forwarding behavior not perfectly matching all iPhone/iPad variants or future iOS releases

Accepted tradeoffs for v1:
- audio playback remains local to `plugin-capture-direct` rather than being surfaced as raw PCM to the host app
- BLE discovery is the default cross-platform path even if some environments may eventually benefit from DNS-SD fallback
- direct sessions remain single-session/single-window in the current host model

## Acceptance Criteria

This work is complete when all of the following are true:
- double-clicking a startable device launches a real UxPlay-backed direct receiver session
- the PC becomes discoverable from iPhone/iPad Screen Mirroring on both Linux and Windows
- connecting from iPhone/iPad produces real live video frames in the session window
- AirPlay audio is ingested and played locally on the host
- startup/readiness only reports direct receiver as ready when the actual bundled runtime is usable
- direct session UI distinguishes waiting, streaming, degraded-audio, and error states
- stopping the session tears down UxPlay, the beacon helper, and local media pipelines cleanly
- existing non-direct capture/control flows continue to function
