"""Module-style ``lattice.workspace`` helper object."""

from __future__ import annotations

from pathlib import Path

from lattice._dataset import DatasetHandle, dataset as load_dataset
from lattice._env import workspace_root
from lattice._propose import propose as write_propose
from lattice._propose import propose_page as write_page


class WorkspaceHelper:
    """Module-style helper: ``lattice.workspace.dataset(...)``."""

    def root(self) -> Path:
        return workspace_root()

    def dataset(self, path: str) -> DatasetHandle:
        return load_dataset(path)

    def propose_page(
        self,
        path: str,
        content: str,
        *,
        summary: str | None = None,
        source_type: str = "task",
        resource: str | None = None,
    ) -> dict:
        return write_page(
            path,
            content,
            summary=summary,
            source_type=source_type,
            resource=resource,
        )

    def propose(
        self,
        *,
        commands: list[dict],
        summary: str,
        source_type: str = "task",
        resource: str | None = None,
        warnings: list[str] | None = None,
    ) -> dict:
        return write_propose(
            commands=commands,
            summary=summary,
            source_type=source_type,
            resource=resource,
            warnings=warnings,
        )
