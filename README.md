# ios-control

Rust workspace for a Linux/Windows host app that coordinates capture, control, and grounding plugins for iOS-control experiments.

This README is intentionally focused on the current developer flow.

## Prerequisites

- Rust stable
- Python 3
- Git

Linux builds also need:

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

Run the workspace tests:

```bash
cargo test --workspace
```

Open the desktop shell:

```bash
cargo run -p host-desktop
```

The desktop app currently opens into a demo shell UI. The real local end-to-end path today is the orchestrator-backed mock session test below.

## End-to-end developer flow

### 1. Build the local mock plugin binaries

```bash
cargo build \
  -p plugin-capture-window \
  -p plugin-control-ble \
  -p plugin-grounding-core
```

Expected result:

- The plugin binaries are built under your active Cargo target directory
- They speak newline-delimited JSON over stdio using protocol version `2`

### 2. Run the local mock end-to-end test

```bash
cargo test -p ios-control-session-orchestrator \
  local_mock_e2e_builds_streaming_session \
  -- --exact
```

Expected result:

- `SessionOrchestrator::start_session_with_plugins(...)` reaches a plugin-backed session and currently ends in `SessionPhase::Degraded`
- The mock session selects capture source `window-1`
- The active session reports `capture.window`, `control.ble`, and `grounding.core`
- The test proves capture/session wiring and shuts the session down cleanly

### 3. Run the desktop shell

```bash
cargo run -p host-desktop
```

Expected result:

- A window titled `iOS Control Host` opens
- The shell renders the current demo state

### 4. Run the broader validation loop

```bash
cargo test --workspace
python3 -m unittest discover -s tests/ci -p 'test_*.py' -v
python3 scripts/assert_ci_release.py full
```

## Real-device validation (current status)

Real-device end-to-end validation is not yet complete on this branch. The only verified flow today is the local mock plugin-backed session described above. Use the acceptance matrix below to track current, verified status and gaps.

See `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md` for the operator-facing matrix and validation checklist.

## Release packaging

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

## Notes

- The orchestrator-backed mock flow is the main local E2E path today.
- The desktop shell is still a demo UI, not a real device-session UI yet.
- Real iPhone/iPad control is not fully implemented yet.
