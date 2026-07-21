"""Injectable Lattice workspace SDK for native/uv notebooks and tasks.

Set ``LATTICE_WORKSPACE`` to the workspace root (task runner and native kernel
inject this). ``PYTHONPATH`` must include this package's parent directory so
``import lattice`` resolves without a separate install.
"""

from __future__ import annotations

from lattice._dataset import DatasetHandle, dataset
from lattice._env import LatticeWorkspaceError, workspace_root
from lattice._propose import propose, propose_page
from lattice._workspace import WorkspaceHelper

__all__ = [
    "DatasetHandle",
    "LatticeWorkspaceError",
    "WorkspaceHelper",
    "dataset",
    "propose",
    "propose_page",
    "workspace",
    "workspace_root",
]

workspace = WorkspaceHelper()
