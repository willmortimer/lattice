"""Resolve the open workspace root from ``LATTICE_WORKSPACE``."""

from __future__ import annotations

import os
from pathlib import Path


class LatticeWorkspaceError(RuntimeError):
    """Raised when the workspace root cannot be resolved."""


def workspace_root() -> Path:
    """Return the absolute workspace root from ``LATTICE_WORKSPACE``.

    Raises:
        LatticeWorkspaceError: when the env var is missing or not a directory.
    """
    raw = os.environ.get("LATTICE_WORKSPACE")
    if not raw or not raw.strip():
        raise LatticeWorkspaceError(
            "LATTICE_WORKSPACE is not set; native tasks and kernels inject it "
            "when Lattice launches Python. Set it to the workspace root to use "
            "the SDK outside Lattice."
        )
    root = Path(raw).expanduser().resolve()
    if not root.is_dir():
        raise LatticeWorkspaceError(
            f"LATTICE_WORKSPACE is not a directory: {root}"
        )
    return root
