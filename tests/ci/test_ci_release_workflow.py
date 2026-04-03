import re
import unittest
from pathlib import Path
from unittest import mock

import scripts.assert_ci_release as assert_ci_release
from scripts.assert_ci_release import assert_validation_structure


REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "ci-release.yml"
CROSS_TOML_PATH = REPO_ROOT / "Cross.toml"


class CiReleaseWorkflowTests(unittest.TestCase):
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
        self.assertIn("needs: [test-native-linux, test-native-windows]", workflow_text)
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
        self.assertIn("--target ${{ matrix.target }}", workflow_text)
        self.assertIn("--bin-dir target/${{ matrix.target }}/release", workflow_text)
        self.assertIn("--out-dir dist/${{ matrix.target }}", workflow_text)
        self.assertIn("--sha ${{ github.sha }}", workflow_text)
        self.assertIn("--ref-name ${{ github.ref_name }}", workflow_text)
        self.assertIn("--run-number ${{ github.run_number }}", workflow_text)
        self.assertIn("--timestamp ${{ steps.build-metadata.outputs.timestamp }}", workflow_text)
        self.assertIn("if-no-files-found: error", workflow_text)

    def test_full_workflow_contains_publish_jobs(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        assert_ci_release.assert_full_workflow(workflow_text)

    def test_full_workflow_rejects_missing_publish_invariants(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        without_prerelease = workflow_text.replace("prerelease: true\n", "", 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(without_prerelease)

        without_needs = workflow_text.replace("needs: [build-release-matrix]\n", "", 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(without_needs)

        without_flatten = workflow_text.replace("mkdir -p upload\n", "", 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(without_flatten)

        without_release_notes = workflow_text.replace("generate_release_notes: true\n", "", 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(without_release_notes)

    def test_full_workflow_rejects_cleanup_with_blanket_or_true(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            "gh release delete rolling-main --yes --cleanup-tag",
            "gh release delete rolling-main --yes --cleanup-tag || true",
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

    def test_full_workflow_rejects_missing_orphan_tag_delete_call(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            "gh api -X DELETE repos/$GH_REPO/git/refs/tags/rolling-main\n",
            "",
            1,
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_wrong_tag_release_identity(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            "tag_name: ${{ github.ref_name }}\n",
            "tag_name: ${{ github.sha }}\n",
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
        mutated = workflow_text.replace("tag_name: rolling-main\n", "tag_name: rolling\n", 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_missing_publish_release_delete(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            "gh release delete rolling-main --yes --cleanup-tag --repo \"$GH_REPO\"\n",
            "",
            1,
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_wrong_publish_main_release_name(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace("          name: rolling-main\n", "          name: rolling\n", 1)
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_wrong_publish_tag_release_name(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace(
            "          name: ${{ github.ref_name }}\n",
            "          name: ${{ github.sha }}\n",
            1,
        )
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_full_workflow(mutated)

    def test_full_workflow_rejects_publish_tag_overwrite(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        mutated = workflow_text.replace("overwrite_files: false\n", "overwrite_files: true\n", 1)
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
