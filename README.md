# ios-control

`ios-control` is a Rust workspace for a Linux/Windows host application that is intended to coordinate iOS screen capture, Bluetooth-based control, and grounding or action-planning plugins.

The repository is currently a host-plus-plugins scaffold. It has:

- A desktop shell in `apps/host-desktop`
- Shared contracts and runtime crates in `crates/*`
- Plugin entry points in `plugins/*`
- A GitHub Actions workflow for native validation, release packaging, and release publishing

It does not yet provide a complete consumer-ready iPhone/iPad remote-control flow. The most useful way to use the repo today is as a developer workspace with a local end-to-end loop around the host shell, mock plugin sessions, unit tests, and release packaging.

## Workspace layout

- `apps/host-desktop`: cross-platform desktop shell built with `eframe`/`egui`
- `crates/contracts`: shared session, capture, control, and grounding contracts
- `crates/plugin-runtime`: host-to-plugin process handshake/runtime support
- `crates/session-orchestrator`: session orchestration and mock end-to-end coverage
- `plugins/control-ble`: Bluetooth control capability probing
- `plugins/capture-window`: window-capture plugin scaffold
- `plugins/capture-direct`: direct receiver scaffold
- `plugins/grounding-core`: coordinate mapping and action-selection logic
- `plugins/mock-device`: minimal mock plugin used for protocol/runtime testing
- `.github/workflows/ci-release.yml`: native CI, multi-arch release build, and publishing workflow

## Current status

What works today:

- The desktop shell builds and opens
- The runtime/plugin roundtrip and orchestrator-backed mock session paths work
- Grounding, capture, contract, and runtime units are covered by tests
- CI and release packaging are implemented and locally verifiable

What is still incomplete:

- No full host UI wiring for live device sessions yet
- No fully integrated real-device iOS control flow yet
- Capture and BLE plugins are still mostly capability probes or scaffolds
- Windows ARM64 and other cross-runner release paths are validated structurally, not by real hosted-runner execution from this README

## Prerequisites

- Rust stable toolchain
- Python 3
- Git

Linux desktop builds also need the native libraries used in CI:

```bash
sudo apt-get update
sudo apt-get install -y \
  libxcb-render0-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libxkbcommon-dev \
  libssl-dev
```

## Quick start

Run the full workspace test suite:

```bash
cargo test --workspace
```

Launch the host desktop shell:

```bash
cargo run -p host-desktop
```

At the moment the host app still opens into a demo shell UI. The runnable local end-to-end path is the orchestrator-backed mock session test below; the desktop app is useful for verifying the shell and panel state, not for controlling a real iPhone yet.

## End-to-end developer flow

This is the current runnable end-to-end path in the repo.

### 1. Build the local mock plugin binaries

```bash
cargo build \
  -p plugin-capture-window \
  -p plugin-control-ble \
  -p plugin-grounding-core
```

Expected result:

- The three plugin binaries build under `target/debug`
- They speak newline-delimited JSON over stdio using protocol version `2`

### 2. Run the focused orchestrator-backed mock end-to-end test

```bash
cargo test -p ios-control-session-orchestrator \
  local_mock_e2e_builds_streaming_session \
  -- --exact
```

Expected result:

- The test builds the plugin binaries if needed
- `SessionOrchestrator::start_session_with_plugins(...)` reaches `SessionPhase::Streaming`
- The mock session selects capture source `window-1`
- The active session reports `capture.window`, `control.ble`, and `grounding.core`
- The session shuts down cleanly at the end of the test

This is the main local E2E verification for the current host/plugin runtime path.

### 3. Run the desktop shell

```bash
cargo run -p host-desktop
```

Expected result:

- A window titled `iOS Control Host` opens
- The dashboard and panel scaffolding render the current demo state

### 4. Run the broader validation loop

```bash
cargo test --workspace
python3 -m unittest discover -s tests/ci -p 'test_*.py' -v
python3 scripts/assert_ci_release.py full
```

What this covers:

- Rust workspace tests
- Orchestrator-backed local mock session coverage
- Capture/control/grounding contract coverage
- Release workflow structure and publish invariants

### 5. Build native release artifacts locally

If you want to exercise the packaging path on your current host architecture, build the release binaries first, then package them.

Linux example:

```bash
cargo build --release \
  --package host-desktop \
  --package plugin-control-ble \
  --package plugin-capture-window \
  --package plugin-capture-direct \
  --package plugin-grounding-core \
  --package plugin-mock-device

python3 scripts/package_release.py \
  --target x86_64-unknown-linux-gnu \
  --bin-dir target/release \
  --out-dir dist/x86_64-unknown-linux-gnu \
  --sha local \
  --ref-name local \
  --run-number 0 \
  --timestamp 1970-01-01T00:00:00Z
```

Expected outputs:

- `dist/x86_64-unknown-linux-gnu/ios-control-x86_64-unknown-linux-gnu.tar.gz`
- `dist/x86_64-unknown-linux-gnu/ios-control-plugins-x86_64-unknown-linux-gnu.tar.gz`

On Windows, use the same packaging script with a Windows target string and the `.exe` binaries in your native `target/release` directory.

## CI and release workflow

The repository now ships a single workflow at `.github/workflows/ci-release.yml`.

It does three things:

- Runs native validation on Linux and Windows
- Builds release artifacts for Linux and Windows, including ARM64 targets
- Publishes rolling `main` artifacts and immutable `v*` tag releases

You can verify the workflow locally without pushing:

```bash
python3 scripts/assert_ci_release.py validation
python3 scripts/assert_ci_release.py build
python3 scripts/assert_ci_release.py full
```

## If you want real device control

This repo is not at that point yet. The intended architecture is:

1. Capture plugin provides the host-visible screen stream
2. Control plugin provides BLE keyboard/pointer transport
3. Grounding plugin maps host coordinates or semantic targets into control actions
4. Host desktop app orchestrates those plugins per device session

Today, the best way to work on that path is:

- Extend the plugin crates first
- Add runtime wiring in `crates/plugin-runtime` and `crates/session-orchestrator`
- Surface the new flow through `apps/host-desktop`
- Keep the orchestrator-backed mock flow and CI checks green while iterating

## Useful commands

```bash
cargo test --workspace
cargo test -p ios-control-session-orchestrator local_mock_e2e_builds_streaming_session -- --exact
cargo run -p host-desktop
python3 -m unittest discover -s tests/ci -p 'test_*.py' -v
python3 scripts/assert_ci_release.py full
```
