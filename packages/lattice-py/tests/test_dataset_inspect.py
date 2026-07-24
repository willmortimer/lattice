"""Tests for bounded DuckDB dataset schema/profile inspection."""

from __future__ import annotations

from pathlib import Path

import pytest

import lattice
from lattice._dataset_inspect import MAX_PROFILE_SAMPLE_ROWS

pytest.importorskip("duckdb")
pa_table = pytest.importorskip("pyarrow").Table


@pytest.fixture()
def workspace(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    monkeypatch.setenv("LATTICE_WORKSPACE", str(tmp_path))
    return tmp_path


def _write_dataset_package(
    root: Path,
    rel_path: str,
    *,
    rows: list[dict[str, object]] | None = None,
) -> lattice.DatasetHandle:
    package = root / rel_path
    package.mkdir(parents=True)
    (package / "dataset.yaml").write_text(
        "format: lattice-dataset\nversion: 1\nid: test\ntitle: Test\n",
        encoding="utf-8",
    )
    if rows is not None:
        facts = package / "facts"
        facts.mkdir(parents=True)
        table = pa_table.from_pylist(rows)
        import pyarrow.parquet as pq

        pq.write_table(table, facts / "sample.parquet")
    return lattice.dataset(rel_path)


def test_schema_empty_facts_is_bounded(workspace: Path) -> None:
    handle = _write_dataset_package(workspace, "Data/Empty.dataset")
    schema = handle.schema()
    assert schema.empty is True
    assert schema.columns == []
    assert schema.relation_sql == ""


def test_schema_and_profile_report_parquet_columns(workspace: Path) -> None:
    handle = _write_dataset_package(
        workspace,
        "Data/Orders.dataset",
        rows=[
            {"order_id": "a", "revenue": 10.5, "units": 2},
            {"order_id": "b", "revenue": 3.0, "units": 1},
        ],
    )

    schema = handle.schema()
    assert schema.empty is False
    column_names = [column.name for column in schema.columns]
    assert "revenue" in column_names
    assert "units" in column_names
    assert schema.relation_sql.startswith("SELECT * FROM read_parquet(")

    profile = handle.profile(sample_rows=100)
    assert profile.sample_rows == 100
    assert profile.profile.row_count == 2
    revenue = next(
        column for column in profile.profile.columns if column.name == "revenue"
    )
    assert revenue.approx_distinct == 2


def test_profile_clamps_sample_rows(workspace: Path) -> None:
    handle = _write_dataset_package(
        workspace,
        "Data/Events.dataset",
        rows=[{"signups": index} for index in range(5)],
    )
    profile = handle.profile(sample_rows=999_999)
    assert profile.sample_rows == MAX_PROFILE_SAMPLE_ROWS


def test_schema_rejects_missing_manifest(workspace: Path) -> None:
    package = workspace / "Data/Broken.dataset"
    package.mkdir(parents=True)
    handle = lattice.dataset("Data/Broken.dataset")
    with pytest.raises(ValueError, match="dataset.yaml"):
        handle.schema()


def test_schema_rejects_escape(workspace: Path) -> None:
    with pytest.raises(ValueError):
        lattice.dataset("../outside.dataset").schema()
