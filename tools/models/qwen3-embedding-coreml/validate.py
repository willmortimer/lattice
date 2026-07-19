#!/usr/bin/env python3
"""Validate Core ML embedding parity and retrieval gates (research stub)."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


def _missing_deps() -> list[str]:
    missing: list[str] = []
    try:
        import coremltools  # noqa: F401
    except ImportError:
        missing.append("coremltools")
    try:
        import numpy  # noqa: F401
    except ImportError:
        missing.append("numpy")
    return missing


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Validate Core ML Qwen3 embeddings against reference outputs."
    )
    parser.add_argument(
        "--mlpackage",
        required=True,
        help="Path to converted .mlpackage.",
    )
    parser.add_argument(
        "--fixtures",
        default="fixtures",
        help="Directory of tokenizer/input fixtures.",
    )
    parser.add_argument(
        "--expected",
        default="expected",
        help="Directory of reference embeddings for parity checks.",
    )
    args = parser.parse_args()

    missing = _missing_deps()
    if missing:
        print(
            "error: missing Python dependencies: "
            + ", ".join(missing)
            + "\n"
            "Install with: pip install -r requirements.txt",
            file=sys.stderr,
        )
        return 2

    mlpackage = Path(args.mlpackage).expanduser().resolve()
    if not mlpackage.exists():
        print(
            f"error: mlpackage does not exist: {mlpackage}\n"
            "Run convert.py after export.py produces an artifact.",
            file=sys.stderr,
        )
        return 3

    fixtures_dir = Path(args.fixtures).expanduser().resolve()
    expected_dir = Path(args.expected).expanduser().resolve()
    if not fixtures_dir.is_dir():
        print(
            f"error: fixtures directory missing: {fixtures_dir}\n"
            "Add tokenizer/input fixtures before running validation.",
            file=sys.stderr,
        )
        return 4

    if not expected_dir.is_dir():
        print(
            f"error: expected directory missing: {expected_dir}\n"
            "Add reference embeddings before running validation.",
            file=sys.stderr,
        )
        return 5

    fixture_files = [p for p in fixtures_dir.iterdir() if p.name != ".gitkeep"]
    if not fixture_files:
        print(
            f"error: no fixture files in {fixtures_dir}\n"
            "Populate fixtures/ with ASCII, Unicode, Markdown, and code samples.",
            file=sys.stderr,
        )
        return 6

    print(
        "error: validate.py is a research stub; parity and retrieval checks are "
        "not implemented yet.\n"
        f"mlpackage: {mlpackage}\n"
        f"fixtures: {fixtures_dir} ({len(fixture_files)} file(s))\n"
        f"expected: {expected_dir}\n"
        "Acceptance gates: embedding parity, retrieval parity (≥98% top-10), "
        "latency, index compatibility, reliability, packaging.\n"
        "Record results in RESULTS.md.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
