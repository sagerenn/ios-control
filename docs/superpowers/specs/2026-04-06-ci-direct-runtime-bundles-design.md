# CI Direct Runtime Bundles Design

**Goal:** Extend the GitHub release workflow so release artifacts include a packaged `runtime/uxplay/<target>` tree for all current release targets, using a hybrid strategy that downloads pinned GStreamer runtimes where official binaries exist and builds missing runtimes from source in CI.

## Scope

This design covers the CI workflow, artifact graph, runtime-bundle production rules, and packaging/test updates needed to include UxPlay and GStreamer runtime trees in release archives.

In scope:
- adding a dedicated CI job matrix to build or assemble direct-runtime bundles
- producing runtime artifacts for all current release targets:
  - `x86_64-unknown-linux-gnu`
  - `aarch64-unknown-linux-gnu`
  - `x86_64-pc-windows-msvc`
  - `aarch64-pc-windows-msvc`
- building UxPlay from source for every target
- using official pinned GStreamer downloads where available
- building missing GStreamer targets from source in CI
- passing runtime artifacts into the existing release packaging step
- updating CI workflow assertions/tests to reflect the new job graph

Out of scope:
- changing the current release target matrix
- removing the existing `build-release-matrix` job
- macOS packaging
- changing how `plugin-capture-direct` resolves its runtime once files are present
- publishing prebuilt runtime bundles outside the existing release archives

## Current Problem

The repository now has code paths that expect a packaged `runtime/uxplay/<target>` tree, but the GitHub workflow still only builds Rust binaries and helper executables. Release archives do not yet contain:

- `uxplay(.exe)`
- a packaged GStreamer runtime tree
- `Bluetooth_LE_beacon/uxplay-beacon.py`
- a direct-runtime `manifest.json`

This means the release artifacts do not actually satisfy the runtime contract used by the new direct receiver implementation.

## Recommended Approach

Add a new job matrix named `build-direct-runtime-matrix` and keep the existing `build-release-matrix` intact.

Recommended dependency graph:

1. `test-native-linux`
2. `test-native-windows`
3. `build-direct-runtime-matrix`
4. `build-release-matrix`
5. `publish-main`
6. `publish-tag`

Dependencies:
- `build-direct-runtime-matrix` needs `[test-native-linux, test-native-windows]`
- `build-release-matrix` needs `[test-native-linux, test-native-windows, build-direct-runtime-matrix]`
- publish jobs continue to need `[build-release-matrix]`

This keeps runtime assembly separate from Rust binary compilation while preserving the existing final packaging flow.

## Artifact Flow

`build-direct-runtime-matrix` should upload one artifact per target:

- `direct-runtime-x86_64-unknown-linux-gnu`
- `direct-runtime-aarch64-unknown-linux-gnu`
- `direct-runtime-x86_64-pc-windows-msvc`
- `direct-runtime-aarch64-pc-windows-msvc`

Each artifact should contain exactly:

- `runtime/uxplay/<target>/manifest.json`
- `runtime/uxplay/<target>/uxplay(.exe)`
- `runtime/uxplay/<target>/gstreamer/...`
- `runtime/uxplay/<target>/Bluetooth_LE_beacon/uxplay-beacon.py`

`build-release-matrix` should download the matching runtime artifact for its target and then call:

```bash
python scripts/package_release.py \
  --target ${{ matrix.target }} \
  --bin-dir target/${{ matrix.target }}/release \
  --out-dir dist/${{ matrix.target }} \
  --runtime-dir runtime \
  --sha ${{ github.sha }} \
  --ref-name ${{ github.ref_name }} \
  --run-number ${{ github.run_number }} \
  --timestamp ${{ steps.build-metadata.outputs.timestamp }}
```

This preserves packaging as the single assembly point for Rust binaries plus runtime payloads.

## Target Strategy

### Linux x86_64

- Build UxPlay from source natively on `ubuntu-latest`
- Download a pinned official GStreamer runtime bundle
- Prune the runtime to the required executables/libs/plugins

### Linux aarch64

- Build UxPlay for `aarch64-unknown-linux-gnu`
- Build GStreamer from source in CI
- Stage the same runtime tree layout as x86_64 Linux

### Windows x86_64

- Build UxPlay from source in the supported Windows/MSYS2 toolchain
- Download a pinned official Windows x86_64 GStreamer runtime bundle
- Prune to the required executables/DLLs/plugins

### Windows aarch64

- Build a native ARM64 `uxplay.exe`
- Build GStreamer from source in CI
- Stage the same runtime tree layout as x86_64 Windows

This hybrid strategy matches the approved requirement:
- always build UxPlay from source
- download pinned GStreamer where official runtimes exist
- build missing GStreamer targets from source

## Runtime Manifest

Every runtime bundle must generate a `manifest.json` consumed by the direct plugin. It should include at least:

- `uxplay_path`
- `gst_launch_path`
- `beacon_helper_path`
- `beacon_script_path`
- `python_path`
- `uxplay_version`
- `gstreamer_version`

All executable/script paths should be relative to `runtime/uxplay/<target>/`.

## CI Job Shape

The new `build-direct-runtime-matrix` rows should include:

- `runner`
- `target`
- `archive_ext`
- `uxplay_builder`
- `gstreamer_source`

Recommended rows:

- `ubuntu-latest` / `x86_64-unknown-linux-gnu`
  - `uxplay_builder: native`
  - `gstreamer_source: download`
- `ubuntu-latest` / `aarch64-unknown-linux-gnu`
  - `uxplay_builder: cross`
  - `gstreamer_source: source`
- `windows-latest` / `x86_64-pc-windows-msvc`
  - `uxplay_builder: msys2`
  - `gstreamer_source: download`
- `windows-latest` / `aarch64-pc-windows-msvc`
  - `uxplay_builder: arm64-msys2-or-clang`
  - `gstreamer_source: source`

Each row should perform:

1. checkout
2. restore dedicated runtime cache
3. capture pinned build metadata
4. fetch UxPlay source at a pinned ref
5. build UxPlay for the matrix target
6. download or build GStreamer depending on `gstreamer_source`
7. stage `uxplay-beacon.py`
8. write `manifest.json`
9. upload `direct-runtime-${{ matrix.target }}`

## Workflow Invariants

The current workflow test suite is strict about:

- existing job names
- release matrix rows
- existing packaging command structure
- publish job invariants

Therefore this design intentionally:

- keeps `build-release-matrix` and its matrix rows unchanged
- adds a new job rather than mutating the existing release matrix shape
- extends packaging inputs instead of replacing the packaging step

## Testing

Required CI-facing verification:

- update workflow assertion helpers to recognize:
  - `build-direct-runtime-matrix`
  - `needs: [test-native-linux, test-native-windows, build-direct-runtime-matrix]` on the release build
  - download of matching runtime artifacts before packaging
- add workflow tests that verify:
  - runtime matrix rows exist for all four targets
  - download vs source-build metadata appears on the correct rows
  - release jobs pass `--runtime-dir`
- keep existing release packaging tests passing with the new runtime-dir behavior

## Risks And Tradeoffs

Primary risks:
- CI time will increase significantly, especially for source-built GStreamer rows
- Windows ARM64 UxPlay/GStreamer source builds are the highest-risk target
- upstream UxPlay build assumptions may differ across target/toolchain combinations
- runtime bundle pruning could accidentally omit required plugins/DLLs

Accepted tradeoffs:
- a more complex workflow graph in exchange for better failure isolation
- maintaining a pinned UxPlay ref and pinned GStreamer versions in workflow/config
- separate runtime caching from Rust build caching

## Acceptance Criteria

This work is complete when:

- GitHub Actions builds a direct runtime bundle artifact for each current release target
- `build-release-matrix` consumes the matching runtime artifact for packaging
- final release archives contain `runtime/uxplay/<target>/...`
- workflow tests/assertions are updated and pass
- the existing release artifact upload and publish jobs still work unchanged from the user-facing perspective
