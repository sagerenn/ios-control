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

Windows builds also need:

- Visual Studio Build Tools with the C++ toolchain
- a normal PowerShell or Developer PowerShell environment

## First-time setup

### Linux

```bash
git clone <repo-url> ios-control
cd ios-control

rustup default stable

sudo apt-get update
sudo apt-get install -y \
  libxcb-render0-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libxkbcommon-dev \
  libssl-dev

cargo test --workspace
```

### Windows

```powershell
git clone <repo-url> ios-control
cd ios-control

rustup default stable

cargo test --workspace
```

Expected result:

- the workspace compiles successfully
- all Rust tests pass
- you can then run the local mock E2E flow below

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

## iPhone / iPad setup

If you want to try the intended real-device flow, prepare the iPhone or iPad first:

1. Keep the device unlocked during setup.
2. Turn on Bluetooth.
3. If you want pointer-style control:
   Enable `Settings > Accessibility > Touch > AssistiveTouch`.
4. If you want keyboard-style navigation:
   Enable `Settings > Accessibility > Keyboards > Full Keyboard Access`.
5. If you want direct screen mirroring:
   Keep the iPhone/iPad and PC on the same local network.
6. If you want window-based screen capture:
   install and launch a third-party mirroring app on the PC and connect the device to that app first.

## How to connect

There are two intended screen paths and one intended control path:

- Window capture:
  Mirror the iPhone/iPad into a third-party desktop app first, then let `ios-control` work against that window.
- Direct receiver:
  Start the receiver on the PC, then choose the PC from the iPhone/iPad `Control Center > Screen Mirroring` picker.
- BLE control:
  Pair the iPhone/iPad with the PC from iOS Bluetooth settings, then use keyboard/pointer semantics from the host.

## Current user-facing status

What is verified today:

- local mock plugin-backed flow
- local desktop shell startup

What is not yet verified today:

- real iPhone/iPad pairing end to end
- real live preview from a physical device
- real BLE control on a physical device
- real reconnect / recovery on a physical device

So, this repository does not yet provide a polished “install app, pair iPhone, start controlling it” end-user flow. The sections above describe the intended setup path, not a fully validated product workflow.

For the current code-verified gap list, see `docs/TODO.md`.

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
- They speak newline-delimited JSON over stdio using protocol version `3`

### 2. Run the local mock end-to-end test

```bash
cargo test -p ios-control-session-orchestrator \
  local_mock_e2e_builds_streaming_session \
  -- --exact
```

Expected result:

- `SessionOrchestrator::start_session_with_plugins(...)` reaches a plugin-backed session and currently ends in `SessionPhase::Degraded`
- The mock session selects capture source `window-helper-1`
- The active session reports `capture.window`, `control.ble`, and `grounding.core`
- The test proves capture/session wiring and shuts the session down cleanly

## E2E config

Current local E2E is configuration-light, but these inputs matter:

- `DISPLAY` or `WAYLAND_DISPLAY`:
  Used by the window-capture runtime probe. For the current orchestrator-backed mock flow, the test harness injects a display signal for you. For manual local experiments outside the tests, you need a real desktop session or an equivalent display env var.
- `IOS_CONTROL_DIRECT_RECEIVER_HELPER`:
  Used by the direct-capture stream path. It must point to an existing executable. The current verified local E2E flow does not require it because it uses `plugin-capture-window`, not `plugin-capture-direct`.
- `CARGO_TARGET_DIR` and `CARGO_BUILD_TARGET`:
  The test/support helpers honor these when locating built plugin binaries. If you override them locally, keep your plugin builds and test runs consistent.

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

## GitHub workflow

The repository uses one workflow at `.github/workflows/ci-release.yml`.

Behavior by event:

- `pull_request`:
  Runs native validation only on Linux and Windows.
  It does not run the full release matrix, package release archives, or publish GitHub releases.
- `push` to `main`:
  Runs native validation, the full release build matrix, release packaging, artifact upload, and rolling release publishing.
- `push` of `v*` tags:
  Runs native validation, the full release build matrix, release packaging, artifact upload, and versioned release publishing.

## Release packaging

Linux example:

```bash
cargo build --release \
  --package host-desktop \
  --package plugin-control-ble \
  --package plugin-control-window-bridge \
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
- Release bundles now include the fallback control plugin `plugin-control-window-bridge`.
- Pending Linux/Windows multi-device and fallback validation rows live in `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`.
- The current verified local flow is still helper-backed and mock-driven, not a manually validated physical-device end-to-end session.
