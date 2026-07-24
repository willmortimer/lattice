"""Bounded DuckDB schema/profile inspection for ``.dataset`` packages."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Any

from lattice._env import workspace_root

if TYPE_CHECKING:
    from lattice._dataset import DatasetHandle

DATASET_MANIFEST = "dataset.yaml"
MAX_PROFILE_SAMPLE_ROWS = 100_000
DEFAULT_PROFILE_SAMPLE_ROWS = 10_000


@dataclass(frozen=True)
class DatasetColumnSchema:
    name: str
    data_type: str
    nullable: bool


@dataclass(frozen=True)
class DatasetSchema:
    path: str
    relation_sql: str
    columns: list[DatasetColumnSchema]
    empty: bool


@dataclass(frozen=True)
class ColumnProfile:
    name: str
    data_type: str
    row_count: int | None = None
    null_percentage: float | None = None
    approx_distinct: int | None = None
    min: str | None = None
    max: str | None = None
    avg: float | None = None
    std: float | None = None
    q25: str | None = None
    q50: str | None = None
    q75: str | None = None


@dataclass(frozen=True)
class RelationProfile:
    row_count: int
    columns: list[ColumnProfile]
    relation_sql: str


@dataclass(frozen=True)
class DatasetProfile:
    path: str
    profile: RelationProfile
    sample_rows: int | None


def _require_duckdb() -> Any:
    try:
        import duckdb
    except ImportError as err:
        raise ImportError(
            "dataset.schema() and dataset.profile() need duckdb. "
            "Install with: pip install duckdb"
        ) from err
    return duckdb


def _sql_string_literal(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def _data_type_name(type_name: str) -> str:
    upper = type_name.upper()
    if upper in {"NULL", "NULLABLE"}:
        return "null"
    if upper in {"BOOLEAN", "BOOL"}:
        return "boolean"
    if upper in {
        "TINYINT",
        "SMALLINT",
        "INTEGER",
        "INT",
        "BIGINT",
        "HUGEINT",
        "UTINYINT",
        "USMALLINT",
        "UINTEGER",
        "UBIGINT",
        "INT8",
        "INT16",
        "INT32",
        "INT64",
        "UINT8",
        "UINT16",
        "UINT32",
        "UINT64",
    }:
        return "int64"
    if upper in {"FLOAT", "DOUBLE", "DECIMAL", "NUMERIC", "REAL", "FLOAT32", "FLOAT64"}:
        return "float64"
    if upper in {
        "VARCHAR",
        "TEXT",
        "STRING",
        "BLOB",
        "UUID",
        "DATE",
        "TIME",
        "TIMESTAMP",
        "TIMESTAMP WITH TIME ZONE",
        "TIMESTAMPTZ",
        "INTERVAL",
        "UTF8",
        "LARGEUTF8",
        "BINARY",
        "LARGEBINARY",
    }:
        return "utf8"
    return type_name


def _facts_dir_has_parquet(facts_dir: Path) -> bool:
    if not facts_dir.is_dir():
        return False
    for path in facts_dir.rglob("*"):
        if path.is_file() and path.suffix.lower() == ".parquet":
            return True
    return False


def _default_facts_sql(package_abs: Path) -> str | None:
    facts_dir = package_abs / "facts"
    if not facts_dir.is_dir() or not _facts_dir_has_parquet(facts_dir):
        return None
    glob = facts_dir.joinpath("**", "*.parquet").as_posix()
    return (
        "SELECT * FROM read_parquet("
        f"{_sql_string_literal(glob)}, hive_partitioning = true, union_by_name = true)"
    )


def _empty_io_message(message: str) -> bool:
    lowered = message.lower()
    return (
        "no files found" in lowered
        or "cannot open file" in lowered
        or "io error" in lowered
    )


def _resolve_relation_sql(handle: DatasetHandle, sql: str | None) -> str | None:
    package_abs = handle.absolute_path
    manifest = package_abs / DATASET_MANIFEST
    if not manifest.is_file():
        raise ValueError(f"dataset package missing {DATASET_MANIFEST}: {handle.path}")

    explicit = (sql or "").strip()
    if explicit:
        return explicit
    return _default_facts_sql(package_abs)


def _open_duckdb(root: Path) -> Any:
    duckdb = _require_duckdb()
    canonical = root.resolve()
    conn = duckdb.connect()
    conn.execute(
        f"SET allowed_directories = [{_sql_string_literal(str(canonical))}]"
    )
    conn.execute("SET enable_external_access = false")
    return conn


def _optional_int(value: Any) -> int | None:
    if value is None:
        return None
    if isinstance(value, bool):
        return int(value)
    if isinstance(value, int):
        return value if value >= 0 else None
    if isinstance(value, float):
        return int(value) if value >= 0 else None
    text = str(value).strip()
    if not text:
        return None
    try:
        parsed = int(float(text))
    except ValueError:
        return None
    return parsed if parsed >= 0 else None


def _optional_float(value: Any) -> float | None:
    if value is None:
        return None
    if isinstance(value, bool):
        return float(value)
    if isinstance(value, (int, float)):
        return float(value)
    text = str(value).strip()
    if not text:
        return None
    try:
        return float(text)
    except ValueError:
        return None


def _optional_str(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value)
    return text if text else None


def _summarize_columns(rows: list[tuple[Any, ...]], columns: list[str]) -> list[ColumnProfile]:
    index = {name.lower(): position for position, name in enumerate(columns)}

    def cell(row: tuple[Any, ...], name: str) -> Any:
        return row[index[name.lower()]]

    profiles: list[ColumnProfile] = []
    for row in rows:
        profiles.append(
            ColumnProfile(
                name=str(cell(row, "column_name")),
                data_type=str(cell(row, "column_type")),
                row_count=_optional_int(cell(row, "count")),
                null_percentage=_optional_float(cell(row, "null_percentage")),
                approx_distinct=_optional_int(cell(row, "approx_unique")),
                min=_optional_str(cell(row, "min")),
                max=_optional_str(cell(row, "max")),
                avg=_optional_float(cell(row, "avg")),
                std=_optional_float(cell(row, "std")),
                q25=_optional_str(cell(row, "q25")),
                q50=_optional_str(cell(row, "q50")),
                q75=_optional_str(cell(row, "q75")),
            )
        )
    return profiles


def get_dataset_schema(handle: DatasetHandle, sql: str | None = None) -> DatasetSchema:
    """Bounded schema snapshot via ``SELECT * FROM (relation) LIMIT 0``."""
    relation_sql = _resolve_relation_sql(handle, sql)
    if relation_sql is None:
        return DatasetSchema(
            path=handle.path,
            relation_sql="",
            columns=[],
            empty=True,
        )

    root = workspace_root()
    conn = _open_duckdb(root)
    describe_sql = f"SELECT * FROM ({relation_sql}) AS _lattice_schema LIMIT 0"
    try:
        result = conn.execute(describe_sql)
        description = result.description or []
        columns = [
            DatasetColumnSchema(
                name=str(field[0]),
                data_type=_data_type_name(str(field[1])),
                nullable=True,
            )
            for field in description
        ]
        return DatasetSchema(
            path=handle.path,
            relation_sql=relation_sql,
            columns=columns,
            empty=False,
        )
    except Exception as err:  # noqa: BLE001 — mirror daemon empty-IO handling
        message = str(err)
        if _empty_io_message(message):
            return DatasetSchema(
                path=handle.path,
                relation_sql=relation_sql,
                columns=[],
                empty=True,
            )
        raise ValueError(message) from err
    finally:
        conn.close()


def profile_dataset(
    handle: DatasetHandle,
    *,
    sample_rows: int | None = None,
    sql: str | None = None,
) -> DatasetProfile:
    """Bounded DuckDB ``SUMMARIZE`` profile (optional sample-row wrap)."""
    base_sql = _resolve_relation_sql(handle, sql)
    if base_sql is None:
        return DatasetProfile(
            path=handle.path,
            profile=RelationProfile(row_count=0, columns=[], relation_sql=""),
            sample_rows=None,
        )

    bounded = (
        DEFAULT_PROFILE_SAMPLE_ROWS
        if sample_rows is None
        else max(1, min(int(sample_rows), MAX_PROFILE_SAMPLE_ROWS))
    )
    relation_sql = f"SELECT * FROM ({base_sql}) AS _lattice_rel LIMIT {bounded}"

    root = workspace_root()
    conn = _open_duckdb(root)
    try:
        count_row = conn.execute(
            f"SELECT COUNT(*) AS n FROM ({relation_sql}) AS _lattice_rel"
        ).fetchone()
        row_count = int(count_row[0]) if count_row and count_row[0] is not None else 0
        if row_count == 0:
            return DatasetProfile(
                path=handle.path,
                profile=RelationProfile(
                    row_count=0,
                    columns=[],
                    relation_sql=relation_sql,
                ),
                sample_rows=bounded,
            )

        summarize_sql = f"SUMMARIZE SELECT * FROM ({relation_sql}) AS _lattice_rel"
        result = conn.execute(summarize_sql)
        rows = result.fetchall()
        columns = _summarize_columns(rows, [field[0] for field in result.description])
        return DatasetProfile(
            path=handle.path,
            profile=RelationProfile(
                row_count=row_count,
                columns=columns,
                relation_sql=relation_sql,
            ),
            sample_rows=bounded,
        )
    except Exception as err:  # noqa: BLE001 — mirror daemon empty-IO handling
        message = str(err)
        if _empty_io_message(message):
            return DatasetProfile(
                path=handle.path,
                profile=RelationProfile(
                    row_count=0,
                    columns=[],
                    relation_sql=relation_sql,
                ),
                sample_rows=bounded,
            )
        raise ValueError(message) from err
    finally:
        conn.close()
