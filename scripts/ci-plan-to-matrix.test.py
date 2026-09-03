#!/usr/bin/env python3
"""Tests for scripts/ci-plan-to-matrix.py."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

_SCRIPT = Path(__file__).with_name("ci-plan-to-matrix.py")
_SPEC = importlib.util.spec_from_file_location("ci_plan_to_matrix", _SCRIPT)
assert _SPEC and _SPEC.loader
_mod = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_mod)


class MatrixForPlanTests(unittest.TestCase):
    def test_selects_real_ci_leaves_not_legacy_aliases(self) -> None:
        plan = {
            "execution_plan": {
                "nodes": [
                    {"id": "rust-fmt-check"},
                    {"id": "rust-validate"},
                    {"id": "rust-clippy"},
                    {"id": "rust-test"},
                    {"id": "desktop-ui-test"},
                    {"id": "desktop-ui-build"},
                    {"id": "generated-theme-check"},
                    {"id": "generated-template-check"},
                    {"id": "flake-check"},
                    {"id": "ci"},
                ]
            },
            "roots": ["ci"],
        }
        matrix = _mod.matrix_for_plan(plan)
        tasks = [row["task"] for row in matrix["include"]]
        self.assertEqual(
            tasks,
            [
                "rust-fmt-check",
                "rust-validate",
                "desktop-ui-test",
                "desktop-ui-build",
                "generated-theme-check",
                "generated-template-check",
                "flake-check",
            ],
        )
        self.assertNotIn("clippy", tasks)
        self.assertNotIn("test", tasks)
        self.assertNotIn("rust-clippy", tasks)
        self.assertNotIn("rust-test", tasks)

    def test_falls_back_to_ci_when_plan_is_empty(self) -> None:
        matrix = _mod.matrix_for_plan({"execution_plan": {"nodes": []}, "roots": []})
        self.assertEqual(matrix, {"include": [{"task": "ci"}]})


if __name__ == "__main__":
    raise SystemExit(unittest.main())
