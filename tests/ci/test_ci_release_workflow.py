import re
import unittest
from pathlib import Path
from unittest import mock

import scripts.assert_ci_release as assert_ci_release
from scripts.assert_ci_release import assert_validation_structure


REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "ci-release.yml"
CROSS_TOML_PATH = REPO_ROOT / "Cross.toml"
BUILD_DIRECT_RUNTIME_LINUX_PATH = REPO_ROOT / "scripts" / "ci" / "build_direct_runtime_linux.sh"
BUILD_DIRECT_RUNTIME_WINDOWS_PATH = REPO_ROOT / "scripts" / "ci" / "build_direct_runtime_windows.ps1"


class CiReleaseWorkflowTests(unittest.TestCase):
    def _extract_runtime_matrix_rows(self, workflow_text: str) -> list[tuple[str, str, str, str]]:
        pattern = re.compile(
            r"- runner: (?P<runner>[^\n]+)\n"
            r"\s+target: (?P<target>[^\n]+)\n"
            r"\s+uxplay_builder: (?P<uxplay_builder>[^\n]+)\n"
            r"\s+gstreamer_source: (?P<gstreamer_source>[^\n]+)"
        )
        return [
            (
                match["runner"],
                match["target"],
                match["uxplay_builder"],
                match["gstreamer_source"],
            )
            for match in pattern.finditer(workflow_text)
        ]

    def _extract_release_matrix_rows(self, workflow_text: str) -> list[tuple[str, str, str, str]]:
        pattern = re.compile(
            r"- runner: (?P<runner>[^\n]+)\n"
            r"\s+target: (?P<target>[^\n]+)\n"
            r"\s+archive_ext: (?P<archive_ext>[^\n]+)\n"
            r"\s+builder: (?P<builder>[^\n]+)"
        )
        return [
            (match["runner"], match["target"], match["archive_ext"], match["builder"])
            for match in pattern.finditer(workflow_text)
        ]

    def test_validation_structure_contains_expected_triggers_jobs_and_cache(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        assert_validation_structure(workflow_text)

    def test_ci_runs_host_runtime_smoke_and_python_doc_tests(self) -> None:
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        self.assertIn(
            "cargo test -p host-desktop runtime_start_session_returns_workspace_snapshot -- --exact",
            workflow,
        )
        self.assertIn(
            "python3 -m unittest discover -s tests/ci -p 'test_*.py' -v",
            workflow,
        )

    def test_release_build_structure_contains_expected_matrix_and_artifacts(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        assert_ci_release.assert_release_build_structure(workflow_text)
        self.assertEqual(
            self._extract_release_matrix_rows(workflow_text),
            [
                ("ubuntu-latest", "x86_64-unknown-linux-gnu", "tar.gz", "cargo"),
                ("ubuntu-latest", "aarch64-unknown-linux-gnu", "tar.gz", "cross"),
                ("windows-latest", "x86_64-pc-windows-msvc", "zip", "cargo"),
                ("windows-latest", "aarch64-pc-windows-msvc", "zip", "cargo"),
            ],
        )
        self.assertIn("if: github.event_name == 'push'", workflow_text)
        self.assertIn(
            "needs: [test-native-linux, test-native-windows, build-direct-runtime-matrix]",
            workflow_text,
        )
        self.assertIn("runs-on: ${{ matrix.runner }}", workflow_text)
        self.assertIn("- runner: ubuntu-latest", workflow_text)
        self.assertIn("- runner: windows-latest", workflow_text)
        self.assertIn("target: x86_64-unknown-linux-gnu", workflow_text)
        self.assertIn("target: aarch64-unknown-linux-gnu", workflow_text)
        self.assertIn("target: x86_64-pc-windows-msvc", workflow_text)
        self.assertIn("target: aarch64-pc-windows-msvc", workflow_text)
        self.assertIn("archive_ext: tar.gz", workflow_text)
        self.assertIn("archive_ext: zip", workflow_text)
        self.assertIn("builder: cargo", workflow_text)
        self.assertIn("builder: cross", workflow_text)
        self.assertIn("fail-fast: false", workflow_text)
        self.assertIn("target: ${{ matrix.target }}", workflow_text)
        self.assertIn("shared-key: release-${{ matrix.target }}", workflow_text)
        self.assertIn("key: release-build", workflow_text)
        self.assertIn(
            "cargo install cross --git https://github.com/cross-rs/cross --rev f86fd03bb70b4c6802847c18087e21391498b0b4 --locked",
            workflow_text,
        )
        self.assertIn("id: build-metadata", workflow_text)
        self.assertIn("timestamp=$(date -u +'%Y-%m-%dT%H:%M:%SZ')", workflow_text)
        self.assertIn('echo "timestamp=$(date -u +\'%Y-%m-%dT%H:%M:%SZ\')" >> "$GITHUB_OUTPUT"', workflow_text)
        self.assertIn('cargo build --release --target "${{ matrix.target }}"', workflow_text)
        self.assertIn('cross build --release --target "${{ matrix.target }}"', workflow_text)
        self.assertIn("--package host-desktop", workflow_text)
        self.assertIn("--package plugin-control-ble", workflow_text)
        self.assertIn("--package plugin-control-window-bridge", workflow_text)
        self.assertIn("--package plugin-capture-window", workflow_text)
        self.assertIn("--package plugin-capture-direct", workflow_text)
        self.assertIn("--package plugin-grounding-core", workflow_text)
        self.assertIn("--package plugin-mock-device", workflow_text)
        self.assertIn("--package ble-helper", workflow_text)
        self.assertIn("--package direct-beacon", workflow_text)
        self.assertIn("--target ${{ matrix.target }}", workflow_text)
        self.assertIn("--bin-dir target/${{ matrix.target }}/release", workflow_text)
        self.assertIn("--out-dir dist/${{ matrix.target }}", workflow_text)
        self.assertIn("--runtime-dir runtime", workflow_text)
        self.assertIn("--sha ${{ github.sha }}", workflow_text)
        self.assertIn("--ref-name ${{ github.ref_name }}", workflow_text)
        self.assertIn("--run-number ${{ github.run_number }}", workflow_text)
        self.assertIn("--timestamp ${{ steps.build-metadata.outputs.timestamp }}", workflow_text)
        self.assertIn("if-no-files-found: error", workflow_text)
        self.assertIn("build-direct-runtime-matrix:", workflow_text)
        self.assertIn("name: Download direct runtime artifact", workflow_text)
        self.assertIn("name: direct-runtime-${{ matrix.target }}", workflow_text)

    def test_release_build_structure_requires_runtime_matrix_and_runtime_dir(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        self.assertIn("build-direct-runtime-matrix:", workflow_text)
        self.assertIn(
            "needs: [test-native-linux, test-native-windows, build-direct-runtime-matrix]",
            workflow_text,
        )
        self.assertIn("name: Download direct runtime artifact", workflow_text)
        self.assertIn("name: direct-runtime-${{ matrix.target }}", workflow_text)
        self.assertIn("--runtime-dir runtime", workflow_text)
        self.assertIn("--package direct-beacon", workflow_text)

    def test_runtime_matrix_rows_match_expected_targets(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        self.assertEqual(
            self._extract_runtime_matrix_rows(workflow_text),
            [
                ("ubuntu-latest", "x86_64-unknown-linux-gnu", "native", "source"),
                ("ubuntu-latest", "aarch64-unknown-linux-gnu", "cross", "source"),
                ("windows-latest", "x86_64-pc-windows-msvc", "msys2", "source"),
                ("windows-latest", "aarch64-pc-windows-msvc", "msys2", "source"),
            ],
        )

    def test_linux_runtime_build_installs_arm64_direct_runtime_dependencies(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        self.assertIn("sudo dpkg --add-architecture arm64", workflow_text)
        self.assertIn("sudo apt-get update", workflow_text)
        self.assertRegex(workflow_text, r"(?m)^\s*libglib2\.0-dev\s*$")
        self.assertRegex(workflow_text, r"(?m)^\s*libglib2\.0-dev:arm64\s*$")
        for package in (
            "libdbus-1-dev",
            "libplist-dev",
            "libasound2-dev",
            "libavahi-compat-libdnssd-dev",
            "libssl-dev",
        ):
            self.assertIn(f"{package}:arm64", workflow_text)

    def test_linux_runtime_build_reconfigures_apt_sources_for_arm64_packages(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        self.assertIn("ports.ubuntu.com/ubuntu-ports", workflow_text)
        self.assertIn("Architectures: amd64", workflow_text)
        self.assertIn("Architectures: arm64", workflow_text)

    def test_linux_runtime_build_script_sets_cross_pkg_config_for_aarch64(self) -> None:
        script_text = BUILD_DIRECT_RUNTIME_LINUX_PATH.read_text(encoding="utf-8")
        self.assertIn("export PKG_CONFIG_ALLOW_CROSS=1", script_text)
        self.assertIn("export PKG_CONFIG_LIBDIR=", script_text)
        self.assertIn("/usr/lib/aarch64-linux-gnu/pkgconfig", script_text)
        self.assertIn("export PKG_CONFIG_SYSROOT_DIR=/", script_text)

    def test_linux_runtime_build_script_builds_gstreamer_before_configuring_uxplay(self) -> None:
        script_text = BUILD_DIRECT_RUNTIME_LINUX_PATH.read_text(encoding="utf-8")
        self.assertIn('gst_pkgconfig_path="${gst_prefix}/lib/pkgconfig:${gst_prefix}/lib64/pkgconfig:${gst_prefix}/share/pkgconfig"', script_text)
        self.assertIn('export PKG_CONFIG_PATH="${gst_pkgconfig_path}${PKG_CONFIG_PATH:+:${PKG_CONFIG_PATH}}"', script_text)
        self.assertLess(
            script_text.index('run_meson install -C "${gst_build}"'),
            script_text.index('cmake "${cmake_args[@]}"'),
        )

    def test_linux_runtime_build_script_bootstraps_a_compatible_meson_for_gstreamer(self) -> None:
        script_text = BUILD_DIRECT_RUNTIME_LINUX_PATH.read_text(encoding="utf-8")
        self.assertIn("resolve_gstreamer_meson_requirement()", script_text)
        self.assertIn("ensure_meson()", script_text)
        self.assertIn('python3 -m pip install --upgrade --disable-pip-version-check --target "${meson_site_packages}"', script_text)
        self.assertIn('python3 -m mesonbuild.mesonmain "$@"', script_text)
        self.assertIn('run_meson "${meson_args[@]}"', script_text)

    def test_windows_runtime_build_script_stages_gstreamer_before_configuring_uxplay(self) -> None:
        script_text = BUILD_DIRECT_RUNTIME_WINDOWS_PATH.read_text(encoding="utf-8")
        self.assertLess(
            script_text.index('git clone --depth 1 --branch $env:GSTREAMER_VERSION https://gitlab.freedesktop.org/gstreamer/gstreamer.git $GstSrc'),
            script_text.index('cmake @cmakeArgs'),
        )

    def test_windows_runtime_build_script_supports_only_source_builds(self) -> None:
        script_text = BUILD_DIRECT_RUNTIME_WINDOWS_PATH.read_text(encoding="utf-8")
        self.assertIn('throw "unsupported Windows GStreamerSource=$GstreamerSource"', script_text)
        self.assertNotIn("Invoke-WebRequest", script_text)
        self.assertNotIn(".msi", script_text)

    def test_windows_runtime_build_script_exports_pkg_config_and_prefix_paths(self) -> None:
        script_text = BUILD_DIRECT_RUNTIME_WINDOWS_PATH.read_text(encoding="utf-8")
        self.assertIn('$env:PKG_CONFIG_PATH =', script_text)
        self.assertIn('$env:CMAKE_PREFIX_PATH =', script_text)
        self.assertIn('-DPKG_CONFIG_EXECUTABLE=', script_text)

    def test_windows_runtime_build_script_resolves_meson_from_msys2_for_source_builds(self) -> None:
        script_text = BUILD_DIRECT_RUNTIME_WINDOWS_PATH.read_text(encoding="utf-8")
        self.assertIn("Resolve-MesonInvocation", script_text)
        self.assertIn("meson executable not found on PATH or in common MSYS2 locations", script_text)
        self.assertIn("C:\\msys64\\clangarm64\\bin\\meson.exe", script_text)
        self.assertIn("C:\\msys64\\clangarm64\\bin\\python.exe", script_text)
        self.assertIn('"meson-script.py"', script_text)
        self.assertIn('"mesonbuild.mesonmain"', script_text)

    def test_windows_runtime_build_script_falls_back_to_python_module_for_meson(self) -> None:
        script_text = BUILD_DIRECT_RUNTIME_WINDOWS_PATH.read_text(encoding="utf-8")
        self.assertIn('import mesonbuild', script_text)
        self.assertIn('@("-m", "mesonbuild.mesonmain")', script_text)

    def test_full_workflow_contains_publish_jobs(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        assert_ci_release.assert_full_workflow(workflow_text)

    def test_publish_tag_uses_idempotent_release_commands(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        self.assertIn('gh release view "$tag" --repo "$GH_REPO"', workflow_text)
        self.assertIn('gh release upload "$tag" upload/* --clobber --repo "$GH_REPO"', workflow_text)
        self.assertIn(
            'gh release create "$tag" upload/* --repo "$GH_REPO" --verify-tag --title "$tag" --generate-notes',
            workflow_text,
        )

    def test_publish_main_uses_idempotent_release_commands(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        self.assertIn('notes="Automated rolling release for the latest successful push to main."', workflow_text)
        self.assertIn('gh api repos/$GH_REPO/git/refs/tags/$tag >/dev/null 2>&1', workflow_text)
        self.assertIn('gh api --method PATCH repos/$GH_REPO/git/refs/tags/$tag -f sha="$GITHUB_SHA" -F force=true >/dev/null', workflow_text)
        self.assertIn('gh api --method POST repos/$GH_REPO/git/refs -f ref="refs/tags/$tag" -f sha="$GITHUB_SHA" >/dev/null', workflow_text)
        self.assertIn('gh release view "$tag" --repo "$GH_REPO" >/dev/null 2>&1', workflow_text)
        self.assertIn('gh release edit "$tag" --repo "$GH_REPO" --title "$tag" --notes "$notes" --prerelease', workflow_text)
        self.assertIn('gh release create "$tag" upload/* --repo "$GH_REPO" --target "$GITHUB_SHA" --title "$tag" --notes "$notes" --prerelease', workflow_text)

    def test_full_workflow_rejects_missing_publish_invariants(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        without_prerelease = workflow_text.replace("--prerelease\n", "\n", 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(without_prerelease)

        without_needs = workflow_text.replace("needs: [build-release-matrix]\n", "", 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(without_needs)

        without_flatten = workflow_text.replace("mkdir -p upload\n", "", 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(without_flatten)

        without_tag_release_create = workflow_text.replace(
            '            gh release create "$tag" upload/* --repo "$GH_REPO" --verify-tag --title "$tag" --generate-notes\n',
            "",
            1,
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(without_tag_release_create)

    def test_full_workflow_rejects_softprops_in_publish_main(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            '            gh release edit "$tag" --repo "$GH_REPO" --title "$tag" --notes "$notes" --prerelease\n',
            "        uses: softprops/action-gh-release@v2\n",
            1,
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_missing_publish_concurrency(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace("concurrency:\n      group: rolling-main\n      cancel-in-progress: true\n", "", 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_missing_publish_flatten_command(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            'find artifacts -type f -print0 | xargs -0 -I {} cp "{}" upload/\n',
            "",
            1,
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_missing_publish_main_ref_patch(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            '            gh api --method PATCH repos/$GH_REPO/git/refs/tags/$tag -f sha="$GITHUB_SHA" -F force=true >/dev/null\n',
            "",
            1,
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_wrong_tag_release_identity(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            '          tag="${GITHUB_REF_NAME}"\n',
            '          tag="${GITHUB_SHA}"\n',
            1,
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_missing_publish_repo_scope(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace("GH_REPO: ${{ github.repository }}\n", "", 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_missing_publish_release_identity(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace('          tag="rolling-main"\n', '          tag="rolling"\n', 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_missing_publish_main_release_edit(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            '            gh release edit "$tag" --repo "$GH_REPO" --title "$tag" --notes "$notes" --prerelease\n',
            "",
            1,
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_wrong_publish_main_release_title(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            '            gh release create "$tag" upload/* --repo "$GH_REPO" --target "$GITHUB_SHA" --title "$tag" --notes "$notes" --prerelease\n',
            '            gh release create "$tag" upload/* --repo "$GH_REPO" --target "$GITHUB_SHA" --title "rolling" --notes "$notes" --prerelease\n',
            1,
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_wrong_publish_tag_release_title(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            '            gh release create "$tag" upload/* --repo "$GH_REPO" --verify-tag --title "$tag" --generate-notes\n',
            '            gh release create "$tag" upload/* --repo "$GH_REPO" --verify-tag --title "$GITHUB_SHA" --generate-notes\n',
            1,
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_missing_publish_tag_clobber(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace("--clobber ", "", 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_validation_rejects_missing_windows_cargo_test(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        before, sep, after = workflow_text.partition("test-native-windows:")
        mutated = before + sep + after.replace("cargo test --workspace\n", "", 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_validation_structure(mutated)

    def test_validation_rejects_missing_linux_cargo_test(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        before, sep, after = workflow_text.partition("test-native-linux:")
        mutated = before + sep + after.replace("cargo test --workspace\n", "", 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_validation_structure(mutated)

    def test_release_build_structure_requires_cross_container_config(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        with mock.patch.object(assert_ci_release.Path, "read_text", return_value=""):
            with self.assertRaises(AssertionError):
                assert_ci_release.assert_release_build_structure(workflow_text)

    def test_release_build_structure_rejects_unpinned_cross_install(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        unpinned = workflow_text.replace(
            "cargo install cross --git https://github.com/cross-rs/cross --rev f86fd03bb70b4c6802847c18087e21391498b0b4 --locked",
            "cargo install cross --git https://github.com/cross-rs/cross --rev f86fd03bb70b4c6802847c18087e21391498b0b4",
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_release_build_structure(unpinned)

    def test_release_build_structure_rejects_missing_fail_fast(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace("fail-fast: false\n", "")
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_release_build_structure(mutated)

    def test_release_build_structure_rejects_missing_builder_guards(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace("if: matrix.builder == 'cross'\n", "")
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_release_build_structure(mutated)

    def test_release_build_structure_rejects_hardcoded_archive_extension(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            ".${{ matrix.archive_ext }}",
            ".tar.gz",
            1,
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_release_build_structure(mutated)

    def test_release_build_structure_rejects_missing_build_linux_deps(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace("if: runner.os == 'Linux'\n", "")
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_release_build_structure(mutated)

    def test_release_build_structure_rejects_swapped_builder_guards(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            "name: Build release binaries with cargo\n        if: matrix.builder == 'cargo'\n",
            "name: Build release binaries with cargo\n        if: matrix.builder == 'cross'\n",
        ).replace(
            "name: Build release binaries with cross\n        if: matrix.builder == 'cross'\n",
            "name: Build release binaries with cross\n        if: matrix.builder == 'cargo'\n",
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_release_build_structure(mutated)

    def test_release_build_structure_rejects_matrix_tuple_drift(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            "target: aarch64-unknown-linux-gnu\n            archive_ext: tar.gz\n            builder: cross",
            "target: aarch64-unknown-linux-gnu\n            archive_ext: zip\n            builder: cross",
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_release_build_structure(mutated)

    def test_release_build_structure_requires_validation_structure(self) -> None:
        with mock.patch.object(assert_ci_release, "RELEASE_BUILD_SNIPPETS", []):
            with mock.patch.object(
                assert_ci_release, "assert_validation_structure", side_effect=AssertionError("validation")
            ):
                with self.assertRaises(AssertionError) as exc:
                    assert_ci_release.assert_release_build_structure("workflow")
                self.assertIn("validation", str(exc.exception))

    def test_cross_toml_configures_aarch64_linux_dependencies(self) -> None:
        self.assertTrue(CROSS_TOML_PATH.exists(), "Cross.toml must exist for cross container dependencies")
        cross_text = CROSS_TOML_PATH.read_text(encoding="utf-8")
        self.assertIn("[target.aarch64-unknown-linux-gnu]", cross_text)
        self.assertIn("dpkg --add-architecture ${CROSS_DEB_ARCH}", cross_text)
        self.assertIn("apt-get update", cross_text)
        self.assertIn("apt-get install -y", cross_text)
        for package in (
            "libxcb-render0-dev",
            "libxcb-shape0-dev",
            "libxcb-xfixes0-dev",
            "libxkbcommon-dev",
            "libssl-dev",
        ):
            self.assertIn(f"{package}:${{CROSS_DEB_ARCH}}", cross_text)

    def test_release_build_structure_rejects_missing_cross_apt_update(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        cross_text = CROSS_TOML_PATH.read_text(encoding="utf-8")
        mutated_cross = cross_text.replace("apt-get update", "")
        with mock.patch.object(assert_ci_release.Path, "read_text", return_value=mutated_cross):
            with self.assertRaises(AssertionError):
                assert_ci_release.assert_release_build_structure(workflow_text)

    def test_release_build_structure_rejects_cross_prebuild_order_drift(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        cross_text = CROSS_TOML_PATH.read_text(encoding="utf-8")
        lines = cross_text.splitlines()
        dpkg_index = next(i for i, line in enumerate(lines) if "dpkg --add-architecture ${CROSS_DEB_ARCH}" in line)
        update_index = next(i for i, line in enumerate(lines) if "apt-get update" in line)
        lines[dpkg_index], lines[update_index] = lines[update_index], lines[dpkg_index]
        mutated_cross = "\n".join(lines) + ("\n" if cross_text.endswith("\n") else "")
        with mock.patch.object(assert_ci_release.Path, "read_text", return_value=mutated_cross):
            with self.assertRaises(AssertionError):
                assert_ci_release.assert_release_build_structure(workflow_text)

    def test_main_uses_first_arg_for_phase_and_defaults_to_full(self) -> None:
        with mock.patch.object(assert_ci_release.Path, "read_text", return_value="workflow"):
            with mock.patch.object(assert_ci_release, "assert_validation_structure") as validation:
                exit_code = assert_ci_release.main(["validation"])
                self.assertEqual(exit_code, 0)
                validation.assert_called_once_with("workflow")

        with mock.patch.object(assert_ci_release.Path, "read_text", return_value="workflow"):
            with mock.patch.object(assert_ci_release, "assert_full_workflow") as full:
                exit_code = assert_ci_release.main([])
                self.assertEqual(exit_code, 0)
                full.assert_called_once_with("workflow")

        with mock.patch.object(assert_ci_release.Path, "read_text", return_value="workflow"):
            with mock.patch.object(assert_ci_release, "assert_release_build_structure") as build:
                exit_code = assert_ci_release.main(["build"])
                self.assertEqual(exit_code, 0)
                build.assert_called_once_with("workflow")


if __name__ == "__main__":
    unittest.main()
