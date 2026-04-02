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
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
    "cargo install cross --git https://github.com/cross-rs/cross --rev f86fd03bb70b4c6802847c18087e21391498b0b4",
    "python scripts/package_release.py",
    "actions/upload-artifact@v4",
    "bundle-${{ matrix.target }}",
    "plugins-${{ matrix.target }}",
]

CROSS_CONFIG_SNIPPETS = [
    "[target.aarch64-unknown-linux-gnu]",
    "dpkg --add-architecture ${CROSS_DEB_ARCH}",
    "libxcb-render0-dev:${CROSS_DEB_ARCH}",
    "libxcb-shape0-dev:${CROSS_DEB_ARCH}",
    "libxcb-xfixes0-dev:${CROSS_DEB_ARCH}",
    "libxkbcommon-dev:${CROSS_DEB_ARCH}",
    "libssl-dev:${CROSS_DEB_ARCH}",
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


def assert_cross_container_config(text: str) -> None:
    _assert_snippets(text, CROSS_CONFIG_SNIPPETS, "cross container config")


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
    assert_release_matrix_rows(text)
    cross_toml_path = Path(__file__).resolve().parents[1] / "Cross.toml"
    cross_toml_text = cross_toml_path.read_text(encoding="utf-8")
    assert_cross_container_config(cross_toml_text)


def assert_full_workflow(text: str) -> None:
    assert_validation_structure(text)
    assert_release_build_structure(text)
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
