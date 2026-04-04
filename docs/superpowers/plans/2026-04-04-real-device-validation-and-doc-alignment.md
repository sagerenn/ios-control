# Real-Device Validation And Doc Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add practical automated smoke coverage, a repeatable manual validation record, and code-aligned documentation rules so the repo can track progress toward a real-device app without overstating current status.

**Architecture:** Keep CI hardware-free by relying on local plugin/helper smoke coverage, and move physical-device proof into explicit operator validation artifacts. Back the status docs with lightweight tests so README, TODO, and historical-plan banners do not drift from the current branch.

**Tech Stack:** Rust tests, Python unittest, GitHub Actions, Markdown docs

---

## File Structure

- `apps/host-desktop/tests/runtime_integration.rs`: smoke coverage for real host runtime startup with local plugins.
- `.github/workflows/ci-release.yml`: run the new smoke and Python doc tests in CI.
- `tests/ci/test_ci_release_workflow.py`: assert the workflow runs the doc and smoke steps.
- `tests/ci/test_docs_status.py`: keep README/TODO and historical-plan banners aligned.
- `README.md`: current-status entry point.
- `docs/TODO.md`: code-verified gap list and plan links.
- `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`: manual validation source of truth.
- `docs/validation/real-device-session-template.md`: operator log template for manual validation runs.

### Task 1: Add Hardware-Free Smoke Coverage And CI Wiring

**Files:**
- Modify: `.github/workflows/ci-release.yml`
- Modify: `tests/ci/test_ci_release_workflow.py`
- Modify: `apps/host-desktop/tests/runtime_integration.rs`
- Test: `tests/ci/test_ci_release_workflow.py`

- [ ] **Step 1: Write the failing CI workflow test**

```python
import unittest
from pathlib import Path


class CiReleaseWorkflowTests(unittest.TestCase):
    def test_ci_runs_host_runtime_smoke_and_python_doc_tests(self) -> None:
        workflow = Path(".github/workflows/ci-release.yml").read_text(encoding="utf-8")
        self.assertIn(
            "cargo test -p host-desktop runtime_start_session_returns_workspace_snapshot -- --exact",
            workflow,
        )
        self.assertIn(
            "python3 -m unittest discover -s tests/ci -p 'test_*.py' -v",
            workflow,
        )
```

- [ ] **Step 2: Run the workflow test to verify it fails**

Run: `python3 -m unittest discover -s tests/ci -p 'test_ci_release_workflow.py' -v`

Expected: FAIL because the workflow does not run the host smoke test or the Python test discovery step yet.

- [ ] **Step 3: Add the smoke test and wire it into CI**

```yaml
- name: Run host runtime smoke test
  run: cargo test -p host-desktop runtime_start_session_returns_workspace_snapshot -- --exact

- name: Run Python CI and docs tests
  run: python3 -m unittest discover -s tests/ci -p 'test_*.py' -v
```

- [ ] **Step 4: Run the workflow test and the host smoke test to verify they pass**

Run: `python3 -m unittest discover -s tests/ci -p 'test_ci_release_workflow.py' -v`

Expected: PASS

Run: `cargo test -p host-desktop runtime_start_session_returns_workspace_snapshot -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci-release.yml \
  tests/ci/test_ci_release_workflow.py \
  apps/host-desktop/tests/runtime_integration.rs
git commit -m "ci: run host runtime smoke and python docs tests"
```

### Task 2: Add Manual Validation Templates And Acceptance Rules

**Files:**
- Create: `docs/validation/real-device-session-template.md`
- Modify: `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`
- Modify: `README.md`
- Modify: `docs/TODO.md`
- Test: `tests/ci/test_docs_status.py`

- [ ] **Step 1: Write the failing docs-status tests**

```python
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
```

- [ ] **Step 2: Run the docs-status test to verify it fails**

Run: `python3 -m unittest discover -s tests/ci -p 'test_docs_status.py' -v`

Expected: FAIL because the doc-status test file does not exist yet.

- [ ] **Step 3: Add the validation template and status-doc guard**

```markdown
# Real-Device Session Validation Record

- Date:
- Host OS:
- Host Bluetooth adapter:
- Capture path:
- Control path:
- Device model:
- Pairing result:
- Live preview result:
- Live control result:
- Recovery result:
- Notes:
```

```python
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
```

- [ ] **Step 4: Run the docs-status tests to verify they pass**

Run: `python3 -m unittest discover -s tests/ci -p 'test_docs_status.py' -v`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add docs/validation/real-device-session-template.md \
  docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md \
  README.md \
  docs/TODO.md \
  tests/ci/test_docs_status.py
git commit -m "docs: add validation template and status doc guards"
```

### Task 3: Keep Acceptance Updates Honest

**Files:**
- Modify: `tests/ci/test_docs_status.py`
- Modify: `docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md`
- Test: `tests/ci/test_docs_status.py`

- [ ] **Step 1: Write the failing acceptance-honesty test**

```python
    def test_verified_acceptance_rows_require_explicit_operator_log_reference(self) -> None:
        matrix = Path(
            "docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md"
        ).read_text(encoding="utf-8")
        for line in matrix.splitlines():
            if "| Verified |" in line and "Local mock flow" not in line:
                self.assertIn("docs/validation/", matrix)
```

- [ ] **Step 2: Run the acceptance-honesty test to verify it fails**

Run: `python3 -m unittest discover -s tests/ci -p 'test_docs_status.py' -v`

Expected: FAIL until the test exists and the matrix includes an explicit rule for validation-log evidence.

- [ ] **Step 3: Add the acceptance-matrix rule and test**

```markdown
Before changing any non-mock row to `Verified`, add a corresponding dated record under `docs/validation/` and link it from the matrix update.
```

```python
    def test_verified_acceptance_rows_require_explicit_operator_log_reference(self) -> None:
        matrix = Path(
            "docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md"
        ).read_text(encoding="utf-8")
        self.assertIn("docs/validation/", matrix)
```

- [ ] **Step 4: Run the full Python CI/doc test suite to verify it passes**

Run: `python3 -m unittest discover -s tests/ci -p 'test_*.py' -v`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add tests/ci/test_docs_status.py \
  docs/superpowers/specs/2026-04-03-real-device-acceptance-matrix.md
git commit -m "test: guard acceptance updates with validation evidence"
```
