import unittest
from pathlib import Path


class DocsStatusTests(unittest.TestCase):
    def test_readme_links_to_todo_and_acceptance_matrix(self) -> None:
        readme = Path("README.md").read_text(encoding="utf-8")
        self.assertIn("docs/TODO.md", readme)
        self.assertIn("real-device-acceptance-matrix.md", readme)

    def test_todo_links_to_plan_docs(self) -> None:
        todo = Path("docs/TODO.md").read_text(encoding="utf-8")
        self.assertIn("2026-04-04-host-runtime-and-operator-workflow.md", todo)
        self.assertIn("2026-04-04-live-preview-capture-transport.md", todo)
        self.assertIn("2026-04-04-control-execution-and-observation.md", todo)
        self.assertIn("2026-04-04-real-device-validation-and-doc-alignment.md", todo)

    def test_validation_template_exists(self) -> None:
        template = Path("docs/validation/real-device-session-template.md")
        self.assertTrue(template.exists())

    def test_historical_plans_are_marked_historical(self) -> None:
        historical_paths = [
            "docs/superpowers/specs/2026-04-03-operator-complete-real-device-app-design.md",
            "docs/superpowers/plans/2026-04-03-operator-complete-real-device-app.md",
            "docs/superpowers/plans/2026-04-03-end-to-end-product-completion.md",
            "docs/superpowers/plans/2026-04-03-real-device-e2e-implementation.md",
            "docs/superpowers/plans/2026-04-03-real-device-e2e-gap-closure.md",
        ]
        for path in historical_paths:
            text = Path(path).read_text(encoding="utf-8")
            self.assertIn("Historical planning artifact.", text)

    def test_verified_acceptance_rows_require_explicit_operator_log_reference(self) -> None:
        matrix = Path(
            "docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md"
        ).read_text(encoding="utf-8")
        self.assertIn(
            "Before changing any non-mock row to `Verified`, add a corresponding dated record under `docs/validation/`",
            matrix,
        )
