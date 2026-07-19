#!/usr/bin/env python3
"""Offline unit checks for metrics.py (no ASR / no fixtures)."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from metrics import edit_rates, path_accuracy, token_accuracy


def main() -> int:
    ref = (
        "Lattice voice dictation should preserve CamelCase identifiers like "
        "AsrManager, file paths such as /Users/will/Developer/lattice, and "
        "punctuation around code."
    )
    hyp = (
        "Lattice voice dictation should preserve camel case identifiers like "
        "ASR Manager, file paths such as users will developer lattice, and "
        "punctuation around code."
    )
    raw = edit_rates(ref, hyp, normalized=False)
    norm = edit_rates(ref, hyp, normalized=True)
    assert 0.0 <= raw.wer <= 1.5, raw
    assert 0.0 <= norm.wer <= 1.0, norm
    assert norm.wer <= raw.wer + 1e-9

    tech = token_accuracy(hyp, ["AsrManager"], case_sensitive=False)
    assert tech.hit == 1 and tech.total == 1, tech

    paths = path_accuracy(hyp, ["/Users/will/Developer/lattice"])
    assert paths.hit == 0 and paths.total == 1, paths

    perfect = edit_rates(ref, ref, normalized=True)
    assert perfect.wer == 0.0 and perfect.cer == 0.0

    print("test_metrics: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
