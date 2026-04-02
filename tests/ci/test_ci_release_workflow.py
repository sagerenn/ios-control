import unittest
from pathlib import Path
from unittest import mock

import scripts.assert_ci_release as assert_ci_release
from scripts.assert_ci_release import assert_validation_structure


REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "ci-release.yml"


class CiReleaseWorkflowTests(unittest.TestCase):
    def test_validation_structure_contains_expected_triggers_jobs_and_cache(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        assert_validation_structure(workflow_text)

    def test_release_build_structure_contains_expected_matrix_and_artifacts(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        assert_ci_release.assert_release_build_structure(workflow_text)
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
        self.assertIn("id: build-metadata", workflow_text)
        self.assertIn("timestamp=$(date -u +'%Y-%m-%dT%H:%M:%SZ')", workflow_text)
        self.assertIn('echo "timestamp=$(date -u +\'%Y-%m-%dT%H:%M:%SZ\')" >> "$GITHUB_OUTPUT"', workflow_text)
        self.assertIn('cargo build --release --target "${{ matrix.target }}"', workflow_text)
        self.assertIn('cross build --release --target "${{ matrix.target }}"', workflow_text)
        self.assertNotIn("--locked", workflow_text)
        self.assertIn("--package host-desktop", workflow_text)
        self.assertIn("--package plugin-control-ble", workflow_text)
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


if __name__ == "__main__":
    unittest.main()
