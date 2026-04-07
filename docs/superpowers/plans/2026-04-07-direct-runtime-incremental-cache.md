# Direct Runtime Incremental Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the direct-runtime GitHub Actions cache reusable by turning the Linux and Windows direct-runtime scripts into incremental builders and invalidating the cache when build logic changes.

**Architecture:** Keep the existing `.runtime-cache/<target>` cache shape and teach each direct-runtime build script to reuse a pinned checkout plus built outputs when they already match the requested refs. Update the workflow cache key to include the workflow and script content so stale build trees are discarded when the build logic changes.

**Tech Stack:** GitHub Actions YAML, Bash, PowerShell, Python `unittest`

---

### Task 1: Add regression coverage for incremental cache behavior

**Files:**
- Modify: `tests/ci/test_ci_release_workflow.py`
- Modify: `scripts/assert_ci_release.py`
- Test: `tests/ci/test_ci_release_workflow.py`

- [ ] **Step 1: Write the failing test assertions**

```python
    def test_direct_runtime_cache_key_includes_build_logic_inputs(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        self.assertIn("hashFiles('.github/workflows/ci-release.yml', 'scripts/ci/build_direct_runtime_linux.sh', 'scripts/ci/build_direct_runtime_windows.ps1')", workflow_text)

    def test_linux_runtime_build_script_reuses_runtime_cache_root(self) -> None:
        script_text = BUILD_DIRECT_RUNTIME_LINUX_PATH.read_text(encoding="utf-8")
        self.assertNotIn('rm -rf "${uxplay_src}" "${uxplay_build}" "${gst_src}" "${gst_build}" "${gst_prefix}" "${meson_site_packages}"', script_text)
        self.assertIn('ensure_git_checkout_at_ref "${uxplay_src}" "${UXPLAY_REF}"', script_text)

    def test_windows_runtime_build_script_reuses_runtime_cache_root(self) -> None:
        script_text = BUILD_DIRECT_RUNTIME_WINDOWS_PATH.read_text(encoding="utf-8")
        self.assertNotIn("Remove-Item $WorkRoot -Recurse -Force", script_text)
        self.assertIn("Ensure-GitCheckoutAtRef", script_text)
```

- [ ] **Step 2: Run test to verify it fails**

Run: `python3 -m unittest tests.ci.test_ci_release_workflow.CiReleaseWorkflowTests.test_direct_runtime_cache_key_includes_build_logic_inputs tests.ci.test_ci_release_workflow.CiReleaseWorkflowTests.test_linux_runtime_build_script_reuses_runtime_cache_root tests.ci.test_ci_release_workflow.CiReleaseWorkflowTests.test_windows_runtime_build_script_reuses_runtime_cache_root -v`

Expected: `FAIL` because the workflow key is still static and both build scripts still delete the cached runtime root.

- [ ] **Step 3: Update shared workflow assertions if needed**

```python
RUNTIME_BUILD_SNIPPETS = [
    "build-direct-runtime-matrix:",
    "direct-runtime-${{ matrix.target }}",
    "uxplay_builder:",
    "gstreamer_source:",
    "scripts/ci/build_direct_runtime_linux.sh",
    "scripts/ci/build_direct_runtime_windows.ps1",
    "hashFiles('.github/workflows/ci-release.yml', 'scripts/ci/build_direct_runtime_linux.sh', 'scripts/ci/build_direct_runtime_windows.ps1')",
]
```

- [ ] **Step 4: Re-run the targeted tests once code is updated**

Run: `python3 -m unittest tests.ci.test_ci_release_workflow.CiReleaseWorkflowTests -v`

Expected: the new incremental-cache assertions pass along with the existing workflow structure checks.

### Task 2: Make the Linux direct-runtime build incremental

**Files:**
- Modify: `scripts/ci/build_direct_runtime_linux.sh`
- Test: `tests/ci/test_ci_release_workflow.py`

- [ ] **Step 1: Add helper functions for reusable checkouts and build-output checks**

```bash
git_head_matches_ref() {
  local repo_path="$1"
  local expected_ref="$2"
  git -C "${repo_path}" rev-parse --verify HEAD >/dev/null 2>&1 || return 1
  local expected_commit
  expected_commit="$(git ls-remote --refs --tags --heads origin "${expected_ref}" | awk 'NR==1 { print $1 }')"
  [[ -n "${expected_commit}" ]] || return 1
  [[ "$(git -C "${repo_path}" rev-parse HEAD)" == "${expected_commit}" ]]
}

ensure_git_checkout_at_ref() {
  local repo_path="$1"
  local repo_url="$2"
  local ref="$3"
  shift 3
  local reset_paths=("$@")

  if [[ -d "${repo_path}/.git" ]] && git_head_matches_ref "${repo_path}" "${ref}"; then
    return
  fi

  rm -rf "${reset_paths[@]}"
  git clone --depth 1 --branch "${ref}" "${repo_url}" "${repo_path}"
}
```

- [ ] **Step 2: Run the targeted test to verify the current script still fails it**

Run: `python3 -m unittest tests.ci.test_ci_release_workflow.CiReleaseWorkflowTests.test_linux_runtime_build_script_reuses_runtime_cache_root -v`

Expected: `FAIL` until the old root-level `rm -rf` is removed and helper-based reuse exists.

- [ ] **Step 3: Replace destructive startup with incremental reuse**

```bash
mkdir -p "${work_root}"

ensure_git_checkout_at_ref "${uxplay_src}" "https://github.com/FDH2/UxPlay.git" "${UXPLAY_REF}" \
  "${uxplay_src}" "${uxplay_build}"

ensure_git_checkout_at_ref "${gst_src}" "https://gitlab.freedesktop.org/gstreamer/gstreamer.git" "${GSTREAMER_VERSION}" \
  "${gst_src}" "${gst_build}" "${gst_prefix}" "${meson_site_packages}"

if [[ ! -f "${gst_prefix}/lib/pkgconfig/gstreamer-1.0.pc" ]]; then
  rm -rf "${gst_build}" "${gst_prefix}"
  run_meson "${meson_args[@]}"
  run_meson compile -C "${gst_build}"
  run_meson install -C "${gst_build}"
fi

if [[ ! -x "${uxplay_build}/uxplay" ]]; then
  rm -rf "${uxplay_build}"
  cmake "${cmake_args[@]}"
  cmake --build "${uxplay_build}" --parallel
fi
```

- [ ] **Step 4: Re-run the targeted Linux workflow tests**

Run: `python3 -m unittest tests.ci.test_ci_release_workflow.CiReleaseWorkflowTests.test_linux_runtime_build_script_reuses_runtime_cache_root tests.ci.test_ci_release_workflow.CiReleaseWorkflowTests.test_linux_runtime_build_script_builds_gstreamer_before_configuring_uxplay tests.ci.test_ci_release_workflow.CiReleaseWorkflowTests.test_linux_runtime_build_script_bootstraps_a_compatible_meson_for_gstreamer -v`

Expected: all selected Linux-script assertions pass.

### Task 3: Make the Windows direct-runtime build incremental

**Files:**
- Modify: `scripts/ci/build_direct_runtime_windows.ps1`
- Test: `tests/ci/test_ci_release_workflow.py`

- [ ] **Step 1: Add checkout and output-detection helpers**

```powershell
function Ensure-GitCheckoutAtRef {
    param(
        [Parameter(Mandatory = $true)][string]$RepoPath,
        [Parameter(Mandatory = $true)][string]$RepoUrl,
        [Parameter(Mandatory = $true)][string]$Ref,
        [string[]]$ResetPaths = @()
    )

    if ((Test-Path (Join-Path $RepoPath ".git")) -and (Test-GitCheckoutMatchesRef -RepoPath $RepoPath -Ref $Ref)) {
        return
    }

    foreach ($resetPath in $ResetPaths) {
        if (Test-Path $resetPath) {
            Remove-Item $resetPath -Recurse -Force
        }
    }
    git clone --depth 1 --branch $Ref $RepoUrl $RepoPath
}
```

- [ ] **Step 2: Run the targeted test to verify the current script still fails it**

Run: `python3 -m unittest tests.ci.test_ci_release_workflow.CiReleaseWorkflowTests.test_windows_runtime_build_script_reuses_runtime_cache_root -v`

Expected: `FAIL` until the script stops deleting `$WorkRoot` and starts using incremental helpers.

- [ ] **Step 3: Reuse cached build/install outputs**

```powershell
New-Item -ItemType Directory -Force -Path $WorkRoot | Out-Null

Ensure-GitCheckoutAtRef -RepoPath $UxPlaySrc -RepoUrl "https://github.com/FDH2/UxPlay.git" -Ref $env:UXPLAY_REF -ResetPaths @($UxPlaySrc, $UxPlayBuild)
Ensure-GitCheckoutAtRef -RepoPath $GstSrc -RepoUrl "https://gitlab.freedesktop.org/gstreamer/gstreamer.git" -Ref $env:GSTREAMER_VERSION -ResetPaths @($GstSrc, $GstBuild, $GstRoot)

if (-not (Test-Path (Join-Path $GstRoot "lib\pkgconfig\gstreamer-1.0.pc"))) {
    Remove-Item $GstBuild, $GstRoot -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $GstRoot | Out-Null
    & $mesonInvocation.Command @mesonSetupArgs
    & $mesonInvocation.Command @($mesonInvocation.Arguments + @("compile", "-C", $GstBuild))
    & $mesonInvocation.Command @($mesonInvocation.Arguments + @("install", "-C", $GstBuild))
}

if (-not (Test-Path (Join-Path $UxPlayBuild "uxplay.exe"))) {
    Remove-Item $UxPlayBuild -Recurse -Force -ErrorAction SilentlyContinue
    cmake @cmakeArgs
    cmake --build $UxPlayBuild --config Release --parallel
}
```

- [ ] **Step 4: Re-run the targeted Windows workflow tests**

Run: `python3 -m unittest tests.ci.test_ci_release_workflow.CiReleaseWorkflowTests.test_windows_runtime_build_script_reuses_runtime_cache_root tests.ci.test_ci_release_workflow.CiReleaseWorkflowTests.test_windows_runtime_build_script_stages_gstreamer_before_configuring_uxplay tests.ci.test_ci_release_workflow.CiReleaseWorkflowTests.test_windows_runtime_build_script_resolves_meson_from_msys2_for_source_builds -v`

Expected: all selected Windows-script assertions pass.

### Task 4: Invalidate cached runtime trees when build logic changes

**Files:**
- Modify: `.github/workflows/ci-release.yml`
- Modify: `tests/ci/test_ci_release_workflow.py`
- Modify: `scripts/assert_ci_release.py`
- Test: `tests/ci/test_ci_release_workflow.py`

- [ ] **Step 1: Update the workflow cache key**

```yaml
      - name: Restore direct runtime cache
        uses: actions/cache@v4
        with:
          path: .runtime-cache/${{ matrix.target }}
          key: direct-runtime-${{ matrix.target }}-${{ env.UXPLAY_REF }}-${{ env.GSTREAMER_VERSION }}-${{ hashFiles('.github/workflows/ci-release.yml', 'scripts/ci/build_direct_runtime_linux.sh', 'scripts/ci/build_direct_runtime_windows.ps1') }}
```

- [ ] **Step 2: Run the targeted cache-key regression test**

Run: `python3 -m unittest tests.ci.test_ci_release_workflow.CiReleaseWorkflowTests.test_direct_runtime_cache_key_includes_build_logic_inputs -v`

Expected: `PASS`

- [ ] **Step 3: Run the full workflow test module**

Run: `python3 -m unittest tests.ci.test_ci_release_workflow -v`

Expected: all workflow assertions pass.

- [ ] **Step 4: Inspect the final diff before reporting**

Run: `git diff -- .github/workflows/ci-release.yml scripts/ci/build_direct_runtime_linux.sh scripts/ci/build_direct_runtime_windows.ps1 tests/ci/test_ci_release_workflow.py scripts/assert_ci_release.py`

Expected: diff shows incremental reuse logic in both scripts, the broadened cache key, and workflow tests/assertions updated for the new behavior.
