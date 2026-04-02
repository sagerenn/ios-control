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

    def test_release_build_structure_requires_validation_structure(self) -> None:
        with self.assertRaises(AssertionError):
            assert_ci_release.assert_release_build_structure("build-release")

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
