#!/usr/bin/env python3

from pathlib import Path
from typing import Sequence
import re
import sys


VALIDATION_SNIPPETS = [
    "name: ci-release",
    "pull_request:",
    "push:",
    "branches:",
    "  - main",
    "tags:",
    '  - "v*"',
    "permissions:",
    "  contents: read",
    "test-native-linux:",
    "runs-on: ubuntu-latest",
    "sudo apt-get update && sudo apt-get install -y libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev",
    "uses: actions-rust-lang/setup-rust-toolchain@v1",
    "cache: false",
    "rustflags: \"\"",
    "uses: Swatinem/rust-cache@v2",
    "shared-key: native-linux",
    "key: cargo-test",
    "cargo test --workspace",
    "test-native-windows:",
    "runs-on: windows-latest",
    "shared-key: native-windows",
]

RELEASE_BUILD_SNIPPETS = [
    "build-release-matrix:",
    "if: github.event_name == 'push'",
    "needs: [test-native-linux, test-native-windows]",
    "fail-fast: false",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
    "runs-on: ${{ matrix.runner }}",
    "if: matrix.builder == 'cargo'",
    "if: matrix.builder == 'cross'",
    "target: ${{ matrix.target }}",
    "shared-key: release-${{ matrix.target }}",
    "key: release-build",
    "cargo install cross --git https://github.com/cross-rs/cross --rev f86fd03bb70b4c6802847c18087e21391498b0b4 --locked",
    "id: build-metadata",
    "timestamp=$(date -u +'%Y-%m-%dT%H:%M:%SZ')",
    "python scripts/package_release.py",
    "--package host-desktop",
    "--package plugin-control-ble",
    "--package plugin-capture-window",
    "--package plugin-capture-direct",
    "--package plugin-grounding-core",
    "--package plugin-mock-device",
    "--bin-dir target/${{ matrix.target }}/release",
    "--out-dir dist/${{ matrix.target }}",
    "--sha ${{ github.sha }}",
    "--ref-name ${{ github.ref_name }}",
    "--run-number ${{ github.run_number }}",
    "--timestamp ${{ steps.build-metadata.outputs.timestamp }}",
    "actions/upload-artifact@v4",
    "bundle-${{ matrix.target }}",
    "plugins-${{ matrix.target }}",
    "path: dist/${{ matrix.target }}/ios-control-${{ matrix.target }}.${{ matrix.archive_ext }}",
    "path: dist/${{ matrix.target }}/ios-control-plugins-${{ matrix.target }}.${{ matrix.archive_ext }}",
    "if-no-files-found: error",
]

CROSS_TARGET = "aarch64-unknown-linux-gnu"
EXPECTED_PREBUILD = [
    "dpkg --add-architecture ${CROSS_DEB_ARCH}",
    "apt-get update",
    "apt-get install -y libxcb-render0-dev:${CROSS_DEB_ARCH} libxcb-shape0-dev:${CROSS_DEB_ARCH} "
    "libxcb-xfixes0-dev:${CROSS_DEB_ARCH} libxkbcommon-dev:${CROSS_DEB_ARCH} libssl-dev:${CROSS_DEB_ARCH}",
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


def _assert_snippets(text: str, snippets: Sequence[str], label: str) -> None:
    missing = [snippet for snippet in snippets if snippet not in text]
    if missing:
        missing_text = "\n".join(f"- {snippet}" for snippet in missing)
        raise AssertionError(f"Missing {label} snippet(s):\n{missing_text}")


def assert_validation_structure(text: str) -> None:
    _assert_snippets(text, VALIDATION_SNIPPETS, "validation")


def _extract_prebuild_commands(text: str, target: str) -> list[str]:
    header = f"[target.{target}]"
    lines = text.splitlines()
    start = None
    for index, line in enumerate(lines):
        if line.strip() == header:
            start = index + 1
            break
    if start is None:
        raise AssertionError(f"Missing cross container config section: {header}")

    end = len(lines)
    for index in range(start, len(lines)):
        stripped = lines[index].strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            end = index
            break
    section = "\n".join(lines[start:end])
    match = re.search(r"^\s*pre-build\s*=\s*\[(?P<body>.*?)\]", section, re.DOTALL | re.MULTILINE)
    if not match:
        raise AssertionError(f"Missing cross container config pre-build list in {header}")
    commands = re.findall(r'"([^"]*)"', match.group("body"))
    if not commands:
        raise AssertionError(f"Empty cross container config pre-build list in {header}")
    return commands


def assert_cross_container_config(text: str) -> None:
    commands = _extract_prebuild_commands(text, CROSS_TARGET)
    if commands != EXPECTED_PREBUILD:
        raise AssertionError(
            "Cross container pre-build commands must match exactly.\n"
            f"Expected: {EXPECTED_PREBUILD}\n"
            f"Actual: {commands}"
        )


def _extract_release_matrix_rows(text: str) -> list[tuple[str, str, str, str]]:
    pattern = (
        r"- runner: (?P<runner>[^\n]+)\n"
        r"\s+target: (?P<target>[^\n]+)\n"
        r"\s+archive_ext: (?P<archive_ext>[^\n]+)\n"
        r"\s+builder: (?P<builder>[^\n]+)"
    )
    return [
        (match["runner"], match["target"], match["archive_ext"], match["builder"])
        for match in re.finditer(pattern, text)
    ]


def _extract_job_block(text: str, job_name: str) -> str:
    header = f"  {job_name}:"
    lines = text.splitlines()
    start = None
    for index, line in enumerate(lines):
        if line == header:
            start = index
            break
    if start is None:
        raise AssertionError(f"Missing job: {job_name}")
    end = len(lines)
    for index in range(start + 1, len(lines)):
        if re.match(r"^  [A-Za-z0-9_-]+:\s*$", lines[index]):
            end = index
            break
    return "\n".join(lines[start:end])


def assert_release_matrix_rows(text: str) -> None:
    expected = [
        ("ubuntu-latest", "x86_64-unknown-linux-gnu", "tar.gz", "cargo"),
        ("ubuntu-latest", "aarch64-unknown-linux-gnu", "tar.gz", "cross"),
        ("windows-latest", "x86_64-pc-windows-msvc", "zip", "cargo"),
        ("windows-latest", "aarch64-pc-windows-msvc", "zip", "cargo"),
    ]
    actual = _extract_release_matrix_rows(text)
    if actual != expected:
        raise AssertionError(f"Release matrix rows must match exactly.\nExpected: {expected}\nActual: {actual}")


def assert_release_build_structure(text: str) -> None:
    assert_validation_structure(text)
    _assert_snippets(text, RELEASE_BUILD_SNIPPETS, "release build")
    _assert_snippets(
        text,
        [
            "name: Install Linux UI dependencies\n        if: runner.os == 'Linux'\n        run: sudo apt-get update && sudo apt-get install -y libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev",
            "name: Build release binaries with cargo\n        if: matrix.builder == 'cargo'\n        shell: bash\n        run: >\n          cargo build --release --target \"${{ matrix.target }}\"\n          --package host-desktop\n          --package plugin-control-ble\n          --package plugin-capture-window\n          --package plugin-capture-direct\n          --package plugin-grounding-core\n          --package plugin-mock-device",
            "name: Build release binaries with cross\n        if: matrix.builder == 'cross'\n        shell: bash\n        run: >\n          cross build --release --target \"${{ matrix.target }}\"\n          --package host-desktop\n          --package plugin-control-ble\n          --package plugin-capture-window\n          --package plugin-capture-direct\n          --package plugin-grounding-core\n          --package plugin-mock-device",
        ],
        "release build step pairing",
    )
    assert_release_matrix_rows(text)
    cross_toml_path = Path(__file__).resolve().parents[1] / "Cross.toml"
    cross_toml_text = cross_toml_path.read_text(encoding="utf-8")
    assert_cross_container_config(cross_toml_text)


def assert_full_workflow(text: str) -> None:
    assert_validation_structure(text)
    assert_release_build_structure(text)
    publish_main = _extract_job_block(text, "publish-main")
    publish_tag = _extract_job_block(text, "publish-tag")

    _assert_snippets(
        publish_main,
        [
            "if: github.event_name == 'push' && github.ref == 'refs/heads/main'",
            "needs: [build-release-matrix]",
            "runs-on: ubuntu-latest",
            "concurrency:",
            "group: rolling-main",
            "cancel-in-progress: true",
            "permissions:",
            "contents: write",
            "GH_REPO: ${{ github.repository }}",
            "actions/download-artifact@v5",
            "mkdir -p upload",
            'find artifacts -type f -print0 | xargs -0 -I {} cp "{}" upload/',
            "upload/",
            "gh release delete rolling-main --yes --cleanup-tag --repo \"$GH_REPO\"",
            "softprops/action-gh-release@v2",
            "tag_name: rolling-main",
            "name: rolling-main",
            "files: upload/*",
            "fail_on_unmatched_files: true",
            "overwrite_files: true",
            "prerelease: true",
            "target_commitish: ${{ github.sha }}",
            "gh api -X DELETE repos/$GH_REPO/git/refs/tags/rolling-main",
        ],
        "publish-main",
    )
    if "|| true" in publish_main:
        raise AssertionError("publish-main cleanup must not use '|| true'")

    _assert_snippets(
        publish_tag,
        [
            "if: github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')",
            "needs: [build-release-matrix]",
            "runs-on: ubuntu-latest",
            "permissions:",
            "contents: write",
            "actions/download-artifact@v5",
            "mkdir -p upload",
            'find artifacts -type f -print0 | xargs -0 -I {} cp "{}" upload/',
            "upload/",
            "softprops/action-gh-release@v2",
            "files: upload/*",
            "fail_on_unmatched_files: true",
            "overwrite_files: true",
            "generate_release_notes: true",
            "tag_name: ${{ github.ref_name }}",
            "name: ${{ github.ref_name }}",
        ],
        "publish-tag",
    )
    _assert_snippets(text, PUBLISH_SNIPPETS, "publish")


def main(argv: Sequence[str]) -> int:
    phase = argv[0] if argv else "full"
    if phase not in {"validation", "build", "full"}:
        print("Usage: python3 scripts/assert_ci_release.py [validation|build|full]", file=sys.stderr)
        return 2

    workflow_path = Path(__file__).resolve().parents[1] / ".github" / "workflows" / "ci-release.yml"
    workflow_text = workflow_path.read_text(encoding="utf-8")

    if phase == "validation":
        assert_validation_structure(workflow_text)
    elif phase == "build":
        assert_release_build_structure(workflow_text)
    else:
        assert_full_workflow(workflow_text)

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
