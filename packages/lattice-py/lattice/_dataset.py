"""Resolve dataset package paths under the workspace root."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from lattice._env import workspace_root


@dataclass(frozen=True)
class DatasetHandle:
    """Workspace-relative dataset location plus resolved absolute path."""

    path: str
    absolute_path: Path
    kind: str = "dataset"

    def exists(self) -> bool:
        return self.absolute_path.exists()

    def sources_dir(self) -> Path:
        return self.absolute_path / "sources"

    def facts_dir(self) -> Path:
        return self.absolute_path / "facts"

    def list_source_files(self) -> list[Path]:
        sources = self.sources_dir()
        if not sources.is_dir():
            return []
        return sorted(p for p in sources.rglob("*") if p.is_file())

    def list_parquet_files(self) -> list[Path]:
        facts = self.facts_dir()
        if not facts.is_dir():
            return []
        return sorted(facts.rglob("*.parquet"))

    def read_table(self):
        """Load tabular data with pyarrow or pandas when installed.

        Prefers Parquet under ``facts/``, then CSV under ``sources/``.
        """
        try:
            import pyarrow.parquet as pq
        except ImportError:
            pq = None  # type: ignore[assignment]

        parquet_files = self.list_parquet_files()
        if parquet_files and pq is not None:
            if len(parquet_files) == 1:
                return pq.read_table(parquet_files[0])
            tables = [pq.read_table(path) for path in parquet_files]
            import pyarrow as pa

            return pa.concat_tables(tables)

        csv_files = [
            path
            for path in self.list_source_files()
            if path.suffix.lower() == ".csv"
        ]
        if csv_files:
            try:
                import pandas as pd
            except ImportError as err:
                raise ImportError(
                    "lattice.dataset(...).read_table() needs pandas (or pyarrow "
                    "for Parquet). Install with: pip install pandas pyarrow"
                ) from err
            frames = [pd.read_csv(path) for path in csv_files]
            if len(frames) == 1:
                return frames[0]
            return pd.concat(frames, ignore_index=True)

        if parquet_files and pq is None:
            raise ImportError(
                "dataset has Parquet under facts/ but pyarrow is not installed. "
                "Install with: pip install pyarrow"
            )

        raise FileNotFoundError(
            f"no Parquet under facts/ or CSV under sources/ for {self.path}"
        )


def dataset(path: str) -> DatasetHandle:
    """Resolve ``path`` under ``LATTICE_WORKSPACE`` and return a handle."""
    root = workspace_root()
    rel = path.strip().replace("\\", "/")
    while rel.startswith("./"):
        rel = rel[2:]
    if not rel or rel.startswith("/") or ".." in rel.split("/"):
        raise ValueError(f"dataset path must be workspace-relative: {path!r}")

    absolute = (root / rel).resolve()
    try:
        absolute.relative_to(root)
    except ValueError as err:
        raise ValueError(f"dataset path escapes workspace root: {path!r}") from err

    return DatasetHandle(path=rel, absolute_path=absolute)
