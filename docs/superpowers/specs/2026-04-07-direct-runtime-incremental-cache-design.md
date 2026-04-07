# Direct Runtime Incremental Cache Design

## Goal

Make the `build-direct-runtime-matrix` GitHub Actions job actually benefit from its existing cache by changing the direct-runtime build scripts from destructive rebuilds to incremental reuse of `.runtime-cache/<target>`.

## Scope

This design covers:

- the direct-runtime cache used by [`.github/workflows/ci-release.yml`](/home/ubuntu/ios-control/.github/workflows/ci-release.yml)
- the Linux direct-runtime build script at [`scripts/ci/build_direct_runtime_linux.sh`](/home/ubuntu/ios-control/scripts/ci/build_direct_runtime_linux.sh)
- the Windows direct-runtime build script at [`scripts/ci/build_direct_runtime_windows.ps1`](/home/ubuntu/ios-control/scripts/ci/build_direct_runtime_windows.ps1)
- workflow cache-key invalidation rules for direct-runtime builds

Out of scope:

- changing release target coverage
- splitting the cache into multiple independent caches
- changing Rust build caching
- publishing prebuilt runtime dependencies outside the workflow cache

## Current Problem

`build-direct-runtime-matrix` already restores `.runtime-cache/<target>` with `actions/cache`, but both direct-runtime build scripts delete the entire target cache directory at startup.

That means every push does all expensive work again:

- cloning UxPlay
- cloning GStreamer
- configuring GStreamer
- compiling GStreamer
- compiling UxPlay

The cache currently stores data that the scripts intentionally discard, so it provides almost no runtime benefit.

## Chosen Approach

Keep the existing cache path and convert the direct-runtime scripts into incremental builders.

Each script should treat `.runtime-cache/<target>` as persistent state with these rules:

1. Reuse cached source trees when they already match the pinned ref or version.
2. Refresh a source tree only when it is missing, invalid, or checked out at the wrong revision.
3. Reuse configured build/install outputs when the expected final artifacts are already present.
4. Rebuild only the missing parts needed to generate the final `runtime/` bundle.
5. Continue generating `runtime/` from scratch on every workflow run so the uploaded artifact is fresh even when inputs are cached.

This keeps the implementation aligned with the existing workflow shape and avoids adding more cache layers than the current CI needs.

## Cache Layout

The direct-runtime cache remains rooted at:

```text
.runtime-cache/<target>/
```

Expected reusable subdirectories:

- `UxPlay`
- `uxplay-build`
- `gstreamer`
- `gstreamer-build`
- `gst-root`
- Linux-only private Meson site-packages when required

The scripts should stop deleting the full cache root. They may still remove or recreate specific subdirectories when a source tree is invalid or when a rebuild is required for consistency.

## Source Reuse Rules

### UxPlay

- If `UxPlay/.git` exists and `HEAD` already matches `UXPLAY_REF`, reuse the checkout.
- Otherwise remove only the UxPlay source/build directories and reclone UxPlay at the pinned ref.

### GStreamer

- If `gstreamer/.git` exists and `HEAD` already matches `GSTREAMER_VERSION`, reuse the checkout.
- Otherwise remove only the GStreamer source/build/install directories and reclone GStreamer at the pinned ref.

This keeps invalidation local to the dependency that changed.

## Build Reuse Rules

### GStreamer

Treat `gst-root` as the installed output cache.

- If the installed tree already contains the expected runtime metadata and pkg-config directories, skip Meson setup/compile/install.
- If the installed tree is missing or incomplete, rebuild GStreamer from the cached or refreshed source tree.
- If the source revision changes, discard `gstreamer-build` and `gst-root` before rebuilding.

### UxPlay

Treat the built executable as the reuse signal.

- Linux: reuse when `uxplay-build/uxplay` exists.
- Windows: reuse when `uxplay-build/Release/uxplay.exe` or the produced executable path used by the current generator exists.
- If the expected executable is missing, rerun CMake configure/build from the cached or refreshed source tree.
- If the source revision changes, discard `uxplay-build` before rebuilding.

## Workflow Cache Key

The workflow should continue caching `.runtime-cache/<target>`, but the key should invalidate when build behavior changes. The key should therefore include:

- target triple
- `UXPLAY_REF`
- `GSTREAMER_VERSION`
- a hash of the direct-runtime workflow and both direct-runtime build scripts

This ensures cached outputs are discarded when the build logic changes in a way that may invalidate the existing tree.

## Failure Handling

The scripts should prefer conservative recovery over partial reuse:

- if a cached checkout is not a valid git repository, remove and recreate it
- if required installed outputs are missing, rebuild instead of trying to patch the tree in place
- if a configure/build step fails after a source refresh, leave the failing job red rather than silently falling back to stale artifacts

This keeps cache behavior predictable and debuggable.

## Testing

Required verification for this change:

- update workflow tests so the direct-runtime cache key reflects the incremental-build inputs
- add or extend tests to verify the workflow still restores `.runtime-cache/<target>`
- add or extend tests to verify the scripts no longer start by deleting the entire direct-runtime cache root
- run the workflow test suite that asserts `ci-release.yml` structure

## Risks And Tradeoffs

Primary risks:

- stale cached build outputs could hide a build-logic change if the cache key is too narrow
- incomplete reuse checks could skip a needed rebuild
- Windows build-output detection may vary by generator layout

Accepted tradeoffs:

- cache reuse logic is slightly more complex than unconditional rebuilds
- the scripts may occasionally choose a full dependency rebuild when state is ambiguous
- the cache remains coarse-grained by target rather than split by dependency

## Acceptance Criteria

This work is complete when:

- the direct-runtime scripts no longer delete `.runtime-cache/<target>` at startup
- unchanged CI pushes can reuse cached UxPlay and GStreamer state for the same target
- changing `UXPLAY_REF`, `GSTREAMER_VERSION`, or direct-runtime build logic invalidates the cache
- workflow tests are updated and pass
- the direct-runtime job still uploads a fresh `runtime/` artifact for each target
