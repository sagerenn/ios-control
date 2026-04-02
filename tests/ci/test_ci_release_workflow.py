import unittest
from pathlib import Path

from scripts.assert_ci_release import assert_validation_structure


REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "ci-release.yml"


class CiReleaseWorkflowTests(unittest.TestCase):
    def test_validation_structure_contains_expected_triggers_jobs_and_cache(self) -> None:
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        assert_validation_structure(workflow_text)


if __name__ == "__main__":
    unittest.main()
