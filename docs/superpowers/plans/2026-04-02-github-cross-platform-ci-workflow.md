# GitHub Cross-Platform CI Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a single GitHub Actions workflow that validates the Rust workspace on Linux and Windows, builds portable multi-architecture release bundles plus plugin-only archives, and publishes artifacts for `main` pushes and `v*` tags with cache-aware jobs.

**Architecture:** Keep the automation in one workflow file so validation, release builds, and publishing share the same trigger and artifact naming scheme. Use two small Python helpers: one packages built binaries into deterministic archives on both Linux and Windows, and one sanity-checks the workflow file locally so CI structure is testable before any workflow run.

**Tech Stack:** GitHub Actions YAML, Rust/Cargo workspace, Python 3 standard library (`argparse`, `pathlib`, `shutil`, `tarfile`, `zipfile`, `tempfile`, `unittest`), `actions/checkout`, `actions-rust-lang/setup-rust-toolchain`, `Swatinem/rust-cache`, `actions/upload-artifact`, `actions/download-artifact`, `softprops/action-gh-release`, `cross`

---

## File Structure

- `.github/workflows/ci-release.yml`: one workflow for PR validation, `main` release builds, and tag release publishing.
- `scripts/package_release.py`: cross-platform helper that stages host/plugin binaries, writes `manifest.txt`, and emits bundle plus plugin-only archives.
- `scripts/assert_ci_release.py`: text-based workflow sanity checker for required triggers, jobs, matrix targets, cache steps, artifact uploads, and publish steps.
- `tests/ci/test_package_release.py`: Python unit test for archive layout and manifest contents.
- `tests/ci/test_ci_release_workflow.py`: Python unit test that tightens from validation-only checks to full workflow checks as the implementation progresses.

### Task 1: Add Cross-Platform Release Packaging Helper

**Files:**
- Create: `scripts/package_release.py`
- Create: `tests/ci/test_package_release.py`

- [ ] **Step 1: Write the failing test**

```python
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from package_release import build_release_bundle, executable_name  # noqa: E402


class PackageReleaseTests(unittest.TestCase):
    def _write_binary(self, directory: Path, name: str) -> None:
        path = directory / name
        path.write_text(f"binary:{name}\n", encoding="utf-8")

    def test_linux_bundle_contains_host_plugins_and_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            bin_dir = temp / "release"
            out_dir = temp / "dist"
            bin_dir.mkdir(parents=True)

            for binary in [
                executable_name("host-desktop", "x86_64-unknown-linux-gnu"),
                executable_name("plugin-control-ble", "x86_64-unknown-linux-gnu"),
                executable_name("plugin-capture-window", "x86_64-unknown-linux-gnu"),
                executable_name("plugin-capture-direct", "x86_64-unknown-linux-gnu"),
                executable_name("plugin-grounding-core", "x86_64-unknown-linux-gnu"),
                executable_name("plugin-mock-device", "x86_64-unknown-linux-gnu"),
            ]:
                self._write_binary(bin_dir, binary)

            result = build_release_bundle(
                target="x86_64-unknown-linux-gnu",
                bin_dir=bin_dir,
                out_dir=out_dir,
                sha="deadbeef",
                ref_name="main",
                run_number="42",
                timestamp="2026-04-02T00:00:00Z",
            )

            self.assertTrue(result["bundle_archive"].exists())
            self.assertTrue(result["plugin_archive"].exists())

            with tarfile.open(result["bundle_archive"], "r:gz") as archive:
                bundle_names = archive.getnames()

            self.assertIn(
                "ios-control-x86_64-unknown-linux-gnu/bin/host-desktop",
                bundle_names,
            )
            self.assertIn(
                "ios-control-x86_64-unknown-linux-gnu/plugins/plugin-grounding-core",
                bundle_names,
            )
            self.assertIn(
                "ios-control-x86_64-unknown-linux-gnu/manifest.txt",
                bundle_names,
            )

            manifest = (
                out_dir / "ios-control-x86_64-unknown-linux-gnu" / "manifest.txt"
            ).read_text(encoding="utf-8")
            self.assertIn("target=x86_64-unknown-linux-gnu", manifest)
            self.assertIn("sha=deadbeef", manifest)

            with tarfile.open(result["plugin_archive"], "r:gz") as archive:
                plugin_names = archive.getnames()

            self.assertIn(
                "ios-control-plugins-x86_64-unknown-linux-gnu/plugins/plugin-mock-device",
                plugin_names,
            )

    def test_windows_bundle_uses_zip_archives_and_exe_suffixes(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            bin_dir = temp / "release"
            out_dir = temp / "dist"
            bin_dir.mkdir(parents=True)

            for binary in [
                executable_name("host-desktop", "aarch64-pc-windows-msvc"),
                executable_name("plugin-control-ble", "aarch64-pc-windows-msvc"),
                executable_name("plugin-capture-window", "aarch64-pc-windows-msvc"),
                executable_name("plugin-capture-direct", "aarch64-pc-windows-msvc"),
                executable_name("plugin-grounding-core", "aarch64-pc-windows-msvc"),
                executable_name("plugin-mock-device", "aarch64-pc-windows-msvc"),
            ]:
                self._write_binary(bin_dir, binary)

            result = build_release_bundle(
                target="aarch64-pc-windows-msvc",
                bin_dir=bin_dir,
                out_dir=out_dir,
                sha="feedface",
                ref_name="v0.1.0",
                run_number="99",
                timestamp="2026-04-02T01:00:00Z",
            )

            self.assertEqual(result["bundle_archive"].suffix, ".zip")
            self.assertEqual(result["plugin_archive"].suffix, ".zip")

            with zipfile.ZipFile(result["bundle_archive"]) as archive:
                bundle_names = archive.namelist()

            self.assertIn(
                "ios-control-aarch64-pc-windows-msvc/bin/host-desktop.exe",
                bundle_names,
            )
            self.assertIn(
                "ios-control-aarch64-pc-windows-msvc/plugins/plugin-control-ble.exe",
                bundle_names,
            )


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
python3 -m unittest discover -s tests/ci -p 'test_package_release.py' -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'package_release'`

- [ ] **Step 3: Write minimal implementation**

```python
# scripts/package_release.py
from __future__ import annotations

import argparse
import shutil
import tarfile
import zipfile
from pathlib import Path

HOST_BINARY = "host-desktop"
PLUGIN_BINARIES = [
    "plugin-control-ble",
    "plugin-capture-window",
    "plugin-capture-direct",
    "plugin-grounding-core",
    "plugin-mock-device",
]


def executable_name(binary_name: str, target: str) -> str:
    if "windows" in target:
        return f"{binary_name}.exe"
    return binary_name


def archive_extension(target: str) -> str:
    if "windows" in target:
        return ".zip"
    return ".tar.gz"


def manifest_text(
    *,
    sha: str,
    ref_name: str,
    target: str,
    run_number: str,
    timestamp: str,
) -> str:
    return "\n".join(
        [
            f"sha={sha}",
            f"ref_name={ref_name}",
            f"target={target}",
            f"run_number={run_number}",
            f"timestamp={timestamp}",
        ]
    ) + "\n"


def _reset_dir(path: Path) -> None:
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True, exist_ok=True)


def _copy_binary(source_dir: Path, destination_dir: Path, binary_name: str, target: str) -> None:
    source = source_dir / executable_name(binary_name, target)
    if not source.exists():
        raise FileNotFoundError(f"missing binary: {source}")
    shutil.copy2(source, destination_dir / source.name)


def _write_archive(source_root: Path, archive_path: Path) -> None:
    archive_path.parent.mkdir(parents=True, exist_ok=True)
    if archive_path.suffix == ".zip":
        with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for file_path in sorted(source_root.rglob("*")):
                if file_path.is_file():
                    archive.write(file_path, file_path.relative_to(source_root.parent))
        return

    with tarfile.open(archive_path, "w:gz") as archive:
        archive.add(source_root, arcname=source_root.name)


def build_release_bundle(
    *,
    target: str,
    bin_dir: Path,
    out_dir: Path,
    sha: str,
    ref_name: str,
    run_number: str,
    timestamp: str,
) -> dict[str, Path]:
    bundle_root = out_dir / f"ios-control-{target}"
    bundle_bin_dir = bundle_root / "bin"
    bundle_plugin_dir = bundle_root / "plugins"

    plugin_root = out_dir / f"ios-control-plugins-{target}"
    plugin_dir = plugin_root / "plugins"

    _reset_dir(bundle_bin_dir)
    _reset_dir(bundle_plugin_dir)
    _reset_dir(plugin_dir)

    _copy_binary(bin_dir, bundle_bin_dir, HOST_BINARY, target)

    for plugin_binary in PLUGIN_BINARIES:
        _copy_binary(bin_dir, bundle_plugin_dir, plugin_binary, target)
        _copy_binary(bin_dir, plugin_dir, plugin_binary, target)

    manifest = manifest_text(
        sha=sha,
        ref_name=ref_name,
        target=target,
        run_number=run_number,
        timestamp=timestamp,
    )
    (bundle_root / "manifest.txt").write_text(manifest, encoding="utf-8")
    (plugin_root / "manifest.txt").write_text(manifest, encoding="utf-8")

    bundle_archive = out_dir / f"{bundle_root.name}{archive_extension(target)}"
    plugin_archive = out_dir / f"{plugin_root.name}{archive_extension(target)}"
    _write_archive(bundle_root, bundle_archive)
    _write_archive(plugin_root, plugin_archive)

    return {
        "bundle_root": bundle_root,
        "plugin_root": plugin_root,
        "bundle_archive": bundle_archive,
        "plugin_archive": plugin_archive,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", required=True)
    parser.add_argument("--bin-dir", required=True)
    parser.add_argument("--out-dir", required=True)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--ref-name", required=True)
    parser.add_argument("--run-number", required=True)
    parser.add_argument("--timestamp", required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    result = build_release_bundle(
        target=args.target,
        bin_dir=Path(args.bin_dir),
        out_dir=Path(args.out_dir),
        sha=args.sha,
        ref_name=args.ref_name,
        run_number=args.run_number,
        timestamp=args.timestamp,
    )
    print(result["bundle_archive"])
    print(result["plugin_archive"])


if __name__ == "__main__":
    main()
```

```python
# tests/ci/test_package_release.py
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from package_release import build_release_bundle, executable_name  # noqa: E402


class PackageReleaseTests(unittest.TestCase):
    def _write_binary(self, directory: Path, name: str) -> None:
        path = directory / name
        path.write_text(f"binary:{name}\n", encoding="utf-8")

    def test_linux_bundle_contains_host_plugins_and_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            bin_dir = temp / "release"
            out_dir = temp / "dist"
            bin_dir.mkdir(parents=True)

            for binary in [
                executable_name("host-desktop", "x86_64-unknown-linux-gnu"),
                executable_name("plugin-control-ble", "x86_64-unknown-linux-gnu"),
                executable_name("plugin-capture-window", "x86_64-unknown-linux-gnu"),
                executable_name("plugin-capture-direct", "x86_64-unknown-linux-gnu"),
                executable_name("plugin-grounding-core", "x86_64-unknown-linux-gnu"),
                executable_name("plugin-mock-device", "x86_64-unknown-linux-gnu"),
            ]:
                self._write_binary(bin_dir, binary)

            result = build_release_bundle(
                target="x86_64-unknown-linux-gnu",
                bin_dir=bin_dir,
                out_dir=out_dir,
                sha="deadbeef",
                ref_name="main",
                run_number="42",
                timestamp="2026-04-02T00:00:00Z",
            )

            self.assertTrue(result["bundle_archive"].exists())
            self.assertTrue(result["plugin_archive"].exists())

            with tarfile.open(result["bundle_archive"], "r:gz") as archive:
                bundle_names = archive.getnames()

            self.assertIn(
                "ios-control-x86_64-unknown-linux-gnu/bin/host-desktop",
                bundle_names,
            )
            self.assertIn(
                "ios-control-x86_64-unknown-linux-gnu/plugins/plugin-grounding-core",
                bundle_names,
            )
            self.assertIn(
                "ios-control-x86_64-unknown-linux-gnu/manifest.txt",
                bundle_names,
            )

            manifest = (
                out_dir / "ios-control-x86_64-unknown-linux-gnu" / "manifest.txt"
            ).read_text(encoding="utf-8")
            self.assertIn("target=x86_64-unknown-linux-gnu", manifest)
            self.assertIn("sha=deadbeef", manifest)

            with tarfile.open(result["plugin_archive"], "r:gz") as archive:
                plugin_names = archive.getnames()

            self.assertIn(
                "ios-control-plugins-x86_64-unknown-linux-gnu/plugins/plugin-mock-device",
                plugin_names,
            )

    def test_windows_bundle_uses_zip_archives_and_exe_suffixes(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            bin_dir = temp / "release"
            out_dir = temp / "dist"
            bin_dir.mkdir(parents=True)

            for binary in [
                executable_name("host-desktop", "aarch64-pc-windows-msvc"),
                executable_name("plugin-control-ble", "aarch64-pc-windows-msvc"),
                executable_name("plugin-capture-window", "aarch64-pc-windows-msvc"),
                executable_name("plugin-capture-direct", "aarch64-pc-windows-msvc"),
                executable_name("plugin-grounding-core", "aarch64-pc-windows-msvc"),
                executable_name("plugin-mock-device", "aarch64-pc-windows-msvc"),
            ]:
                self._write_binary(bin_dir, binary)

            result = build_release_bundle(
                target="aarch64-pc-windows-msvc",
                bin_dir=bin_dir,
                out_dir=out_dir,
                sha="feedface",
                ref_name="v0.1.0",
                run_number="99",
                timestamp="2026-04-02T01:00:00Z",
            )

            self.assertEqual(result["bundle_archive"].suffix, ".zip")
            self.assertEqual(result["plugin_archive"].suffix, ".zip")

            with zipfile.ZipFile(result["bundle_archive"]) as archive:
                bundle_names = archive.namelist()

            self.assertIn(
                "ios-control-aarch64-pc-windows-msvc/bin/host-desktop.exe",
                bundle_names,
            )
            self.assertIn(
                "ios-control-aarch64-pc-windows-msvc/plugins/plugin-control-ble.exe",
                bundle_names,
            )


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
python3 -m unittest discover -s tests/ci -p 'test_package_release.py' -v
```

Expected: PASS with `OK`

- [ ] **Step 5: Commit**

```bash
git add scripts/package_release.py tests/ci/test_package_release.py
git commit -m "chore: add release packaging helper"
```

### Task 2: Add Workflow Assertions And Native Validation Jobs

**Files:**
- Create: `scripts/assert_ci_release.py`
- Create: `.github/workflows/ci-release.yml`
- Create: `tests/ci/test_ci_release_workflow.py`

- [ ] **Step 1: Write the failing test**

```python
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from assert_ci_release import assert_validation_structure  # noqa: E402

WORKFLOW = ROOT / ".github" / "workflows" / "ci-release.yml"


class CiReleaseWorkflowTests(unittest.TestCase):
    def test_validation_phase_has_expected_triggers_jobs_and_cache(self) -> None:
        workflow_text = WORKFLOW.read_text(encoding="utf-8")
        assert_validation_structure(workflow_text)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
python3 -m unittest discover -s tests/ci -p 'test_ci_release_workflow.py' -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'assert_ci_release'`

- [ ] **Step 3: Write minimal implementation**

```python
# scripts/assert_ci_release.py
from __future__ import annotations

import sys
from pathlib import Path

VALIDATION_SNIPPETS = [
    "pull_request:",
    "branches:",
    "- main",
    "- \"v*\"",
    "test-native-linux:",
    "test-native-windows:",
    "actions-rust-lang/setup-rust-toolchain@v1",
    "Swatinem/rust-cache@v2",
    "cargo test --workspace",
]

RELEASE_BUILD_SNIPPETS = [
    "build-release-matrix:",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
    "cargo install cross --git https://github.com/cross-rs/cross",
    "python scripts/package_release.py",
    "actions/upload-artifact@v4",
    "bundle-${{ matrix.target }}",
    "plugins-${{ matrix.target }}",
]

PUBLISH_SNIPPETS = [
    "publish-main:",
    "publish-tag:",
    "actions/download-artifact@v5",
    "softprops/action-gh-release@v2",
    "rolling-main",
    "startsWith(github.ref, 'refs/tags/v')",
    "contents: write",
]


def _assert_snippets(text: str, snippets: list[str], label: str) -> None:
    missing = [snippet for snippet in snippets if snippet not in text]
    if missing:
        formatted = "\n".join(f"- {snippet}" for snippet in missing)
        raise AssertionError(f"missing {label} snippets:\n{formatted}")


def assert_validation_structure(text: str) -> None:
    _assert_snippets(text, VALIDATION_SNIPPETS, "validation")


def assert_release_build_structure(text: str) -> None:
    assert_validation_structure(text)
    _assert_snippets(text, RELEASE_BUILD_SNIPPETS, "release build")


def assert_full_workflow(text: str) -> None:
    assert_release_build_structure(text)
    _assert_snippets(text, PUBLISH_SNIPPETS, "publish")


def main(argv: list[str]) -> int:
    phase = argv[0] if argv else "full"
    workflow_path = Path(".github/workflows/ci-release.yml")
    workflow_text = workflow_path.read_text(encoding="utf-8")

    if phase == "validation":
        assert_validation_structure(workflow_text)
    elif phase == "build":
        assert_release_build_structure(workflow_text)
    elif phase == "full":
        assert_full_workflow(workflow_text)
    else:
        raise SystemExit(f"unknown phase: {phase}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
```

```yaml
# .github/workflows/ci-release.yml
name: ci-release

on:
  pull_request:
  push:
    branches:
      - main
    tags:
      - "v*"

permissions:
  contents: read

jobs:
  test-native-linux:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - name: Install Linux UI dependencies
        run: sudo apt-get update && sudo apt-get install -y libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
      - uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          cache: false
          rustflags: ""
      - uses: Swatinem/rust-cache@v2
        with:
          shared-key: native-linux
          key: cargo-test
      - name: Run workspace tests
        run: cargo test --workspace

  test-native-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v5
      - uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          cache: false
          rustflags: ""
      - uses: Swatinem/rust-cache@v2
        with:
          shared-key: native-windows
          key: cargo-test
      - name: Run workspace tests
        run: cargo test --workspace
```

```python
# tests/ci/test_ci_release_workflow.py
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from assert_ci_release import assert_validation_structure  # noqa: E402

WORKFLOW = ROOT / ".github" / "workflows" / "ci-release.yml"


class CiReleaseWorkflowTests(unittest.TestCase):
    def test_validation_phase_has_expected_triggers_jobs_and_cache(self) -> None:
        workflow_text = WORKFLOW.read_text(encoding="utf-8")
        assert_validation_structure(workflow_text)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
python3 -m unittest discover -s tests/ci -p 'test_ci_release_workflow.py' -v
python3 scripts/assert_ci_release.py validation
```

Expected: both commands exit `0`, the unittest run ends with `OK`, and the assertion script prints nothing

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci-release.yml scripts/assert_ci_release.py tests/ci/test_ci_release_workflow.py
git commit -m "ci: add native validation workflow skeleton"
```

### Task 3: Add Release Matrix Builds And Artifact Uploads

**Files:**
- Modify: `.github/workflows/ci-release.yml`
- Modify: `tests/ci/test_ci_release_workflow.py`

- [ ] **Step 1: Write the failing test**

```python
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from assert_ci_release import assert_release_build_structure  # noqa: E402

WORKFLOW = ROOT / ".github" / "workflows" / "ci-release.yml"


class CiReleaseWorkflowTests(unittest.TestCase):
    def test_release_build_matrix_covers_all_targets_and_artifacts(self) -> None:
        workflow_text = WORKFLOW.read_text(encoding="utf-8")
        assert_release_build_structure(workflow_text)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
python3 -m unittest discover -s tests/ci -p 'test_ci_release_workflow.py' -v
```

Expected: FAIL with `AssertionError: missing release build snippets`

- [ ] **Step 3: Write minimal implementation**

```yaml
# .github/workflows/ci-release.yml
name: ci-release

on:
  pull_request:
  push:
    branches:
      - main
    tags:
      - "v*"

permissions:
  contents: read

jobs:
  test-native-linux:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - name: Install Linux UI dependencies
        run: sudo apt-get update && sudo apt-get install -y libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
      - uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          cache: false
          rustflags: ""
      - uses: Swatinem/rust-cache@v2
        with:
          shared-key: native-linux
          key: cargo-test
      - name: Run workspace tests
        run: cargo test --workspace

  test-native-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v5
      - uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          cache: false
          rustflags: ""
      - uses: Swatinem/rust-cache@v2
        with:
          shared-key: native-windows
          key: cargo-test
      - name: Run workspace tests
        run: cargo test --workspace

  build-release-matrix:
    if: github.event_name == 'push'
    needs:
      - test-native-linux
      - test-native-windows
    strategy:
      fail-fast: false
      matrix:
        include:
          - runner: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            archive_ext: tar.gz
            builder: cargo
          - runner: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            archive_ext: tar.gz
            builder: cross
          - runner: windows-latest
            target: x86_64-pc-windows-msvc
            archive_ext: zip
            builder: cargo
          - runner: windows-latest
            target: aarch64-pc-windows-msvc
            archive_ext: zip
            builder: cargo
    runs-on: ${{ matrix.runner }}
    steps:
      - uses: actions/checkout@v5
      - name: Install Linux UI dependencies
        if: runner.os == 'Linux'
        run: sudo apt-get update && sudo apt-get install -y libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
      - uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          cache: false
          rustflags: ""
          target: ${{ matrix.target }}
      - uses: Swatinem/rust-cache@v2
        with:
          shared-key: release-${{ matrix.target }}
          key: release-build
      - name: Install cross
        if: matrix.builder == 'cross'
        run: cargo install cross --git https://github.com/cross-rs/cross
      - name: Capture build timestamp
        id: build-metadata
        shell: bash
        run: echo "timestamp=$(date -u +'%Y-%m-%dT%H:%M:%SZ')" >> "$GITHUB_OUTPUT"
      - name: Build release binaries with cargo
        if: matrix.builder == 'cargo'
        shell: bash
        run: |
          cargo build --release --target "${{ matrix.target }}" \
            -p host-desktop \
            -p plugin-control-ble \
            -p plugin-capture-window \
            -p plugin-capture-direct \
            -p plugin-grounding-core \
            -p plugin-mock-device
      - name: Build release binaries with cross
        if: matrix.builder == 'cross'
        shell: bash
        run: |
          cross build --release --target "${{ matrix.target }}" \
            -p host-desktop \
            -p plugin-control-ble \
            -p plugin-capture-window \
            -p plugin-capture-direct \
            -p plugin-grounding-core \
            -p plugin-mock-device
      - name: Package release archives
        shell: bash
        run: >
          python scripts/package_release.py
          --target "${{ matrix.target }}"
          --bin-dir "target/${{ matrix.target }}/release"
          --out-dir "dist/${{ matrix.target }}"
          --sha "${{ github.sha }}"
          --ref-name "${{ github.ref_name }}"
          --run-number "${{ github.run_number }}"
          --timestamp "${{ steps.build-metadata.outputs.timestamp }}"
      - name: Upload bundle archive
        uses: actions/upload-artifact@v4
        with:
          name: bundle-${{ matrix.target }}
          path: dist/${{ matrix.target }}/ios-control-${{ matrix.target }}.${{ matrix.archive_ext }}
          if-no-files-found: error
      - name: Upload plugin archive
        uses: actions/upload-artifact@v4
        with:
          name: plugins-${{ matrix.target }}
          path: dist/${{ matrix.target }}/ios-control-plugins-${{ matrix.target }}.${{ matrix.archive_ext }}
          if-no-files-found: error
```

```python
# tests/ci/test_ci_release_workflow.py
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from assert_ci_release import assert_release_build_structure  # noqa: E402

WORKFLOW = ROOT / ".github" / "workflows" / "ci-release.yml"


class CiReleaseWorkflowTests(unittest.TestCase):
    def test_release_build_matrix_covers_all_targets_and_artifacts(self) -> None:
        workflow_text = WORKFLOW.read_text(encoding="utf-8")
        assert_release_build_structure(workflow_text)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
python3 -m unittest discover -s tests/ci -p 'test_*.py' -v
python3 scripts/assert_ci_release.py build
```

Expected: both commands exit `0`, the unittest run ends with `OK`, and the assertion script prints nothing

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci-release.yml tests/ci/test_ci_release_workflow.py
git commit -m "ci: add release matrix artifact packaging"
```

### Task 4: Add Rolling Main And Version Tag Publishing

**Files:**
- Modify: `.github/workflows/ci-release.yml`
- Modify: `tests/ci/test_ci_release_workflow.py`

- [ ] **Step 1: Write the failing test**

```python
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from assert_ci_release import assert_full_workflow  # noqa: E402

WORKFLOW = ROOT / ".github" / "workflows" / "ci-release.yml"


class CiReleaseWorkflowTests(unittest.TestCase):
    def test_full_workflow_covers_validation_build_and_publish(self) -> None:
        workflow_text = WORKFLOW.read_text(encoding="utf-8")
        assert_full_workflow(workflow_text)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
python3 -m unittest discover -s tests/ci -p 'test_ci_release_workflow.py' -v
```

Expected: FAIL with `AssertionError: missing publish snippets`

- [ ] **Step 3: Write minimal implementation**

```yaml
# .github/workflows/ci-release.yml
name: ci-release

on:
  pull_request:
  push:
    branches:
      - main
    tags:
      - "v*"

permissions:
  contents: read

jobs:
  test-native-linux:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - name: Install Linux UI dependencies
        run: sudo apt-get update && sudo apt-get install -y libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
      - uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          cache: false
          rustflags: ""
      - uses: Swatinem/rust-cache@v2
        with:
          shared-key: native-linux
          key: cargo-test
      - name: Run workspace tests
        run: cargo test --workspace

  test-native-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v5
      - uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          cache: false
          rustflags: ""
      - uses: Swatinem/rust-cache@v2
        with:
          shared-key: native-windows
          key: cargo-test
      - name: Run workspace tests
        run: cargo test --workspace

  build-release-matrix:
    if: github.event_name == 'push'
    needs:
      - test-native-linux
      - test-native-windows
    strategy:
      fail-fast: false
      matrix:
        include:
          - runner: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            archive_ext: tar.gz
            builder: cargo
          - runner: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            archive_ext: tar.gz
            builder: cross
          - runner: windows-latest
            target: x86_64-pc-windows-msvc
            archive_ext: zip
            builder: cargo
          - runner: windows-latest
            target: aarch64-pc-windows-msvc
            archive_ext: zip
            builder: cargo
    runs-on: ${{ matrix.runner }}
    steps:
      - uses: actions/checkout@v5
      - name: Install Linux UI dependencies
        if: runner.os == 'Linux'
        run: sudo apt-get update && sudo apt-get install -y libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
      - uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          cache: false
          rustflags: ""
          target: ${{ matrix.target }}
      - uses: Swatinem/rust-cache@v2
        with:
          shared-key: release-${{ matrix.target }}
          key: release-build
      - name: Install cross
        if: matrix.builder == 'cross'
        run: cargo install cross --git https://github.com/cross-rs/cross
      - name: Capture build timestamp
        id: build-metadata
        shell: bash
        run: echo "timestamp=$(date -u +'%Y-%m-%dT%H:%M:%SZ')" >> "$GITHUB_OUTPUT"
      - name: Build release binaries with cargo
        if: matrix.builder == 'cargo'
        shell: bash
        run: |
          cargo build --release --target "${{ matrix.target }}" \
            -p host-desktop \
            -p plugin-control-ble \
            -p plugin-capture-window \
            -p plugin-capture-direct \
            -p plugin-grounding-core \
            -p plugin-mock-device
      - name: Build release binaries with cross
        if: matrix.builder == 'cross'
        shell: bash
        run: |
          cross build --release --target "${{ matrix.target }}" \
            -p host-desktop \
            -p plugin-control-ble \
            -p plugin-capture-window \
            -p plugin-capture-direct \
            -p plugin-grounding-core \
            -p plugin-mock-device
      - name: Package release archives
        shell: bash
        run: >
          python scripts/package_release.py
          --target "${{ matrix.target }}"
          --bin-dir "target/${{ matrix.target }}/release"
          --out-dir "dist/${{ matrix.target }}"
          --sha "${{ github.sha }}"
          --ref-name "${{ github.ref_name }}"
          --run-number "${{ github.run_number }}"
          --timestamp "${{ steps.build-metadata.outputs.timestamp }}"
      - name: Upload bundle archive
        uses: actions/upload-artifact@v4
        with:
          name: bundle-${{ matrix.target }}
          path: dist/${{ matrix.target }}/ios-control-${{ matrix.target }}.${{ matrix.archive_ext }}
          if-no-files-found: error
      - name: Upload plugin archive
        uses: actions/upload-artifact@v4
        with:
          name: plugins-${{ matrix.target }}
          path: dist/${{ matrix.target }}/ios-control-plugins-${{ matrix.target }}.${{ matrix.archive_ext }}
          if-no-files-found: error

  publish-main:
    if: github.ref == 'refs/heads/main'
    needs: build-release-matrix
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - name: Download release artifacts
        uses: actions/download-artifact@v5
        with:
          path: release-assets
      - name: Flatten release artifacts
        shell: bash
        run: |
          mkdir -p upload
          find release-assets -type f -exec cp {} upload/ \;
          ls -1 upload
      - name: Remove previous rolling release
        env:
          GH_TOKEN: ${{ github.token }}
        run: gh release delete rolling-main --yes --cleanup-tag || true
      - name: Publish rolling main release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: rolling-main
          name: Rolling main
          target_commitish: ${{ github.sha }}
          prerelease: true
          fail_on_unmatched_files: true
          overwrite_files: true
          files: |
            upload/*
          body: |
            Automated rolling release for the latest successful push to main.

  publish-tag:
    if: startsWith(github.ref, 'refs/tags/v')
    needs: build-release-matrix
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - name: Download release artifacts
        uses: actions/download-artifact@v5
        with:
          path: release-assets
      - name: Flatten release artifacts
        shell: bash
        run: |
          mkdir -p upload
          find release-assets -type f -exec cp {} upload/ \;
          ls -1 upload
      - name: Publish versioned release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: ${{ github.ref_name }}
          name: ${{ github.ref_name }}
          target_commitish: ${{ github.sha }}
          fail_on_unmatched_files: true
          overwrite_files: true
          generate_release_notes: true
          files: |
            upload/*
```

```python
# tests/ci/test_ci_release_workflow.py
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from assert_ci_release import assert_full_workflow  # noqa: E402

WORKFLOW = ROOT / ".github" / "workflows" / "ci-release.yml"


class CiReleaseWorkflowTests(unittest.TestCase):
    def test_full_workflow_covers_validation_build_and_publish(self) -> None:
        workflow_text = WORKFLOW.read_text(encoding="utf-8")
        assert_full_workflow(workflow_text)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
python3 -m unittest discover -s tests/ci -p 'test_*.py' -v
python3 scripts/assert_ci_release.py full
cargo test --workspace
```

Expected: the unittest run ends with `OK`, the workflow assertion exits `0` with no output, and `cargo test --workspace` ends with `test result: ok`

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci-release.yml tests/ci/test_ci_release_workflow.py
git commit -m "ci: publish rolling and tagged releases"
```
