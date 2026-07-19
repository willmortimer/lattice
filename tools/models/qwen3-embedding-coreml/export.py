#!/usr/bin/env python3
"""Export a Qwen3 embedding wrapper for Core ML conversion (research stub)."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


def _missing_deps() -> list[str]:
    missing: list[str] = []
    try:
        import torch  # noqa: F401
    except ImportError:
        missing.append("torch")
    try:
        import transformers  # noqa: F401
    except ImportError:
        missing.append("transformers")
    return missing


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Export Qwen3 embedding graph for Core ML conversion."
    )
    parser.add_argument(
        "--model-path",
        default=None,
        help="Local path to Qwen3-Embedding-0.6B (or set QWEN3_EMBEDDING_MODEL_PATH).",
    )
    parser.add_argument(
        "--out",
        default="build/exported.pt",
        help="Output path for the traced/exported module.",
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

    model_path = args.model_path or __import__("os").environ.get(
        "QWEN3_EMBEDDING_MODEL_PATH"
    )
    if not model_path:
        print(
            "error: no model path provided.\n"
            "Set QWEN3_EMBEDDING_MODEL_PATH or pass --model-path to a local "
            "Qwen/Qwen3-Embedding-0.6B checkout.\n"
            "This research stub does not download models.",
            file=sys.stderr,
        )
        return 3

    resolved = Path(model_path).expanduser().resolve()
    if not resolved.exists():
        print(
            f"error: model path does not exist: {resolved}\n"
            "Download the model separately and point --model-path at it.",
            file=sys.stderr,
        )
        return 4

    print(
        "error: export.py is a research stub; PyTorch export is not implemented yet.\n"
        f"Resolved model path: {resolved}\n"
        f"Intended output: {args.out}\n"
        "See README.md for the planned export wrapper (input_ids, attention_mask).",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
