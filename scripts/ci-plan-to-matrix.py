#!/usr/bin/env python3
"""Convert `nxr ci plan --json` into a GitHub Actions matrix of client leaves.

Usage:
  nix run .#nxr -- ci plan --base origin/main --json | ./scripts/ci-plan-to-matrix.py
  ./scripts/ci-plan-to-matrix.py plan.json
"""

from __future__ import annotations

import json
import sys

# Parallelizable leaves under lattice `ci`. Clippy + tests share one leaf so a
# fresh GHA VM does not compile DuckDB/Wasmtime/Arrow twice.
CLIENT_LEAVES = (
    "rust-fmt-check",
    "rust-validate",
    "desktop-ui-test",
    "desktop-ui-build",
    "generated-theme-check",
    "generated-template-check",
    "flake-check",
)


def load_plan(argv: list[str]) -> dict:
    if len(argv) >= 2 and argv[1] != "-":
        with open(argv[1], encoding="utf-8") as fh:
            return json.load(fh)
    return json.load(sys.stdin)


def matrix_for_plan(plan: dict) -> dict:
    nodes = {n["id"] for n in plan.get("execution_plan", {}).get("nodes", [])}
    roots = set(plan.get("roots") or [])
    selected = [name for name in CLIENT_LEAVES if name in nodes]
    if not selected:
        selected = sorted(roots) if roots else ["ci"]
    return {"include": [{"task": name} for name in selected]}


def main() -> int:
    plan = load_plan(sys.argv)
    matrix = matrix_for_plan(plan)
    json.dump(matrix, sys.stdout, separators=(",", ":"))
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
