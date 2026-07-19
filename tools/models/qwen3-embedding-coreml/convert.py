#!/usr/bin/env python3
"""Convert an exported Qwen3 embedding module to Core ML (research stub)."""

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
        import torch  # noqa: F401
    except ImportError:
        missing.append("torch")
    return missing


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Convert exported Qwen3 embedding graph to Core ML ML Program."
    )
    parser.add_argument(
        "--input",
        required=True,
        help="Path to exported PyTorch artifact from export.py.",
    )
    parser.add_argument(
        "--out",
        default="build/qwen3-embedding.mlpackage",
        help="Output .mlpackage directory.",
    )
    parser.add_argument(
        "--min-deployment-target",
        default="macOS14",
        help="Core ML minimum deployment target (default: macOS14).",
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

    input_path = Path(args.input).expanduser().resolve()
    if not input_path.exists():
        print(
            f"error: input artifact does not exist: {input_path}\n"
            "Run export.py first after providing a local model checkout.",
            file=sys.stderr,
        )
        return 3

    print(
        "error: convert.py is a research stub; coremltools conversion is not "
        "implemented yet.\n"
        f"Input: {input_path}\n"
        f"Intended output: {args.out}\n"
        f"Deployment target: {args.min_deployment_target}\n"
        "See README.md for shape buckets (128/512/2048) and Float16 targets.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
