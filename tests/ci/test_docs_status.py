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

    def test_todo_current_reality_mentions_partial_runtime_wiring(self) -> None:
        todo = Path("docs/TODO.md").read_text(encoding="utf-8")
        self.assertIn("## Current Reality", todo)
        current_reality = todo.split("## Current Reality", 1)[1]
        self.assertIn("apps/host-desktop", current_reality)
        self.assertIn("orchestrator", current_reality.lower())
        self.assertNotIn("not wired directly to the session orchestrator", todo)

    def test_acceptance_matrix_only_allows_non_mock_verified_rows_with_validation_records(self) -> None:
        matrix = Path(
            "docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md"
        ).read_text(encoding="utf-8")
        self.assertIn(
            "Every non-mock `Verified` row must include an inline `docs/validation/...` link to its matching dated validation record.",
            matrix,
        )
        for line in matrix.splitlines():
            stripped = line.strip()
            if not (stripped.startswith("|") and stripped.endswith("|")):
                continue
            cells = [cell.strip() for cell in stripped.strip("|").split("|")]
            if len(cells) < 2 or all(set(cell) <= {"-"} for cell in cells):
                continue
            is_verified = cells[-1].lower() == "verified"
            is_mock_row = "mock" in stripped.lower()
            if is_verified and not is_mock_row:
                self.assertIn("docs/validation/", stripped.lower())
