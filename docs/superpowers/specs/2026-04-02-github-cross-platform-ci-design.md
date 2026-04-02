# GitHub Cross-Platform CI Design

## Goal

Add a GitHub Actions pipeline that validates the Rust workspace on Linux and Windows, builds multi-architecture release artifacts with caching, and publishes portable host-app bundles plus plugin-binary artifacts on pushes to `main` and version tags.

## Project Context

This repository is a Cargo workspace with:

- One desktop application binary: `host-desktop`
- Multiple plugin binaries:
  - `plugin-control-ble`
  - `plugin-capture-window`
  - `plugin-capture-direct`
  - `plugin-grounding-core`
  - `plugin-mock-device`

There is currently no existing [`.github`](/home/ubuntu/ios-control/.github) directory, no GitHub Actions workflow, and no installer-packaging metadata such as WiX, NSIS, AppImage, `.deb`, or desktop-entry assets. The design therefore treats portable archives as the first-pass distributable bundle format.

## Requirements

- Run automated validation on pull requests and pushes to `main`
- Build on both Linux and Windows
- Produce release outputs for multiple architectures
- Use dependency/build caching to keep CI reasonably fast
- Upload host app bundles and plugin binaries as artifacts
- Publish artifacts on every push to `main`
- Publish artifacts on version tags
- Keep the implementation maintainable and debuggable

## Chosen Approach

Use a two-phase GitHub Actions workflow:

1. Validation phase
   - Runs on pull requests and pushes to `main`
   - Uses native GitHub-hosted runners:
     - `ubuntu-latest`
     - `windows-latest`
   - Executes workspace tests with Rust caching

2. Release phase
   - Runs on pushes to `main` and pushes of version tags
   - Builds release artifacts for:
     - `x86_64-unknown-linux-gnu`
     - `aarch64-unknown-linux-gnu`
     - `x86_64-pc-windows-msvc`
     - `aarch64-pc-windows-msvc`
   - Packages the host app and plugin binaries into per-target bundles
   - Uploads artifacts to the workflow run
   - Publishes them to GitHub Releases

This keeps PR CI fast and clear while still providing distributable outputs from integration branches and release tags.

## Workflow Structure

Create a single workflow file, likely [`.github/workflows/ci-release.yml`](/home/ubuntu/ios-control/.github/workflows/ci-release.yml), with these jobs:

### 1. `test-native-linux`

- Triggered for PRs and qualifying pushes
- Runs on `ubuntu-latest`
- Installs stable Rust
- Restores Rust cache
- Runs `cargo test`

### 2. `test-native-windows`

- Triggered for PRs and qualifying pushes
- Runs on `windows-latest`
- Installs stable Rust
- Restores Rust cache
- Runs `cargo test`

### 3. `build-release-matrix`

- Runs only after native validation succeeds
- Runs only on push events to `main` and tag pushes
- Uses a matrix over target triples and packaging metadata
- Produces release builds and staging directories
- Uploads artifacts per target

### 4. `publish-main`

- Runs only on pushes to `main`
- Collects built artifacts
- Publishes or updates a rolling release channel for `main`

### 5. `publish-tag`

- Runs only on version tags such as `v*`
- Publishes a versioned GitHub Release using the same artifacts

## Build Matrix

The release matrix should include:

| OS runner | Rust target | Purpose |
|---|---|---|
| `ubuntu-latest` | `x86_64-unknown-linux-gnu` | Native Linux release |
| `ubuntu-latest` | `aarch64-unknown-linux-gnu` | Linux ARM64 release |
| `windows-latest` | `x86_64-pc-windows-msvc` | Native Windows release |
| `windows-latest` | `aarch64-pc-windows-msvc` | Windows ARM64 release |

Implementation guidance:

- Linux x86_64 can use regular `cargo build --release`
- Linux ARM64 should use a cross-compilation helper
- The preferred first-pass tool is `cross`
- Windows x86_64 should use regular `cargo build --release`
- Windows ARM64 should attempt direct Cargo build after installing the ARM64 target

## Caching Strategy

Use [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache) or an equivalent action for Rust-target-aware caching.

Cache boundaries should separate:

- Native Linux test builds
- Native Windows test builds
- Linux release builds by target triple
- Windows release builds by target triple

Recommended cache key dimensions:

- Runner OS
- Rust target triple
- `Cargo.lock`
- Workflow file hash

This avoids one target’s build artifacts polluting another target’s cache while still reusing dependencies efficiently.

## Artifact Layout

For each release target, stage a directory with this structure:

```text
ios-control-<target>/
  bin/
    host-desktop[.exe]
  plugins/
    plugin-control-ble[.exe]
    plugin-capture-window[.exe]
    plugin-capture-direct[.exe]
    plugin-grounding-core[.exe]
    plugin-mock-device[.exe]
  manifest.txt
```

`manifest.txt` should include:

- Git commit SHA
- Ref name
- Target triple
- Workflow run number
- Build timestamp

Archive formats:

- Linux targets: `.tar.gz`
- Windows targets: `.zip`

Additionally, produce plugin-only archives so plugin binaries are also directly consumable as separate artifacts.

## Publishing Model

### Pull Requests

- Run validation jobs only
- No artifact publishing

### Pushes to `main`

- Run validation
- Run release build matrix
- Upload workflow artifacts
- Publish to a rolling GitHub Release channel for `main`

The `main` release channel may be implemented as:

- A GitHub Release with a stable tag such as `main`
- Assets replaced on each successful `main` push

### Version Tags

- Run validation
- Run release build matrix
- Publish a versioned GitHub Release for the tag

Tag pattern:

- Start with `v*`

## Packaging Boundary

This first pass does **not** attempt true OS-native installers.

Out of scope for this design:

- MSI packaging
- NSIS or Inno Setup installers
- AppImage
- `.deb` or `.rpm`
- platform-specific signing or notarization

Reason:

- The repository currently lacks the metadata, assets, and packaging definitions required to make those formats maintainable.

Instead, the release output is a portable bundle archive that contains:

- the host app binary
- all plugin binaries
- lightweight build metadata

This satisfies the requested “bundles where practical” requirement without inventing an installer subsystem inside CI.

## Failure Handling And Debuggability

The workflow should keep release steps isolated and legible:

- Validation failures should fail before any release job starts
- Each target triple should build in its own matrix leg
- Packaging should happen after successful build in the same leg
- Artifact names should include the target triple
- Publish jobs should only consume already-built artifacts

This makes matrix failures easy to localize and rerun.

## Risks

### Windows ARM64

`aarch64-pc-windows-msvc` is the riskiest target in the initial matrix. Hosted Windows environments may require extra linker/toolchain setup beyond adding the Rust target.

Mitigation:

- Keep that target isolated in its own matrix row
- Make artifact packaging conditional on build success
- Preserve simple output paths and logs for debugging

### Cross-Compilation Drift

Cross-compiled Linux ARM64 builds may behave differently from native Linux x86_64 builds.

Mitigation:

- Keep native test jobs as the correctness gate
- Treat non-native release builds as packaging/build assurance, not a substitute for native runtime testing

## Testing Strategy

The CI implementation should be validated with:

- A workflow schema/sanity test where practical
- A local or CI-targeted test that verifies the workflow file contains:
  - expected triggers
  - native validation jobs
  - release matrix targets
  - Rust cache usage
  - artifact upload and release publish steps

At runtime, correctness is established by:

- `cargo test` on Linux
- `cargo test` on Windows
- release builds succeeding for each matrix target

## Expected Outcome

After implementation:

- PRs will get Linux and Windows Rust test coverage
- Pushes to `main` will produce downloadable cross-platform artifacts
- Version tags will produce versioned release artifacts
- The workflow will be cache-aware and maintainable
- Packaging will remain intentionally portable rather than pretending to support full native installers
