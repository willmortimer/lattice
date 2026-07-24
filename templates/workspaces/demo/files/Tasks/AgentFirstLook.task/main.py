"""First Look agent seed: inspect Orders/Events datasets → propose CRM interface."""

from __future__ import annotations

import json
from dataclasses import asdict
from pathlib import Path

import lattice

TASK = "Tasks/AgentFirstLook.task"
INTERFACE_PATH = "CRM.data/interfaces/AgentDigest.interface.yaml"
ORDERS_PATH = "Data/Orders.dataset"
EVENTS_PATH = "Data/Events.dataset"
PROFILE_SAMPLE_ROWS = 500


def _column_names(schema: lattice.DatasetSchema) -> list[str]:
    return [column.name for column in schema.columns]


def _pick_metric_column(
  schema: lattice.DatasetSchema,
  profile: lattice.DatasetProfile,
  preferred: list[str],
) -> str | None:
    names = set(_column_names(schema))
    for candidate in preferred:
        if candidate in names:
            return candidate
    numeric_types = {"BIGINT", "DOUBLE", "HUGEINT", "INTEGER", "REAL", "int64", "float64"}
    for column in profile.profile.columns:
        if column.name in names and column.data_type.upper() in numeric_types:
            return column.name
    return None


def inspect_dataset(path: str) -> dict[str, object]:
    handle = lattice.dataset(path)
    schema = handle.schema()
    profile = handle.profile(sample_rows=PROFILE_SAMPLE_ROWS)
    metric_column = _pick_metric_column(schema, profile, _metric_preferences(path))
    return {
        "path": path,
        "schema": {
            "empty": schema.empty,
            "columns": [asdict(column) for column in schema.columns],
            "relationSql": schema.relation_sql,
        },
        "profile": {
            "rowCount": profile.profile.row_count,
            "sampleRows": profile.sample_rows,
            "columns": [asdict(column) for column in profile.profile.columns],
        },
        "metricColumn": metric_column,
    }


def _metric_preferences(path: str) -> list[str]:
    if path.endswith("Orders.dataset"):
        return ["revenue", "units"]
    if path.endswith("Events.dataset"):
        return ["signups"]
    return []


def _parquet_read_sql(dataset_path: str) -> str:
    return (
        f"SELECT * FROM read_parquet('{dataset_path}/facts/**/*.parquet', "
        "hive_partitioning = true, union_by_name = true)"
    )


def build_interface_yaml(
    orders: dict[str, object],
    events: dict[str, object],
) -> str:
    orders_metric = str(orders.get("metricColumn") or "revenue")
    events_metric = str(events.get("metricColumn") or "signups")
    orders_cols = ", ".join(
        column["name"] for column in orders["schema"]["columns"]  # type: ignore[index]
    ) or "unknown"
    events_cols = ", ".join(
        column["name"] for column in events["schema"]["columns"]  # type: ignore[index]
    ) or "unknown"
    return f"""format: lattice-interface
version: 1
name: AgentDigest
title: Agent digest
description: |
  Proposed by {TASK} after DuckDB schema/profile inspection of Orders and Events.
  Orders columns: {orders_cols}
  Events columns: {events_cols}
layout:
  columns: 12
components:
  - id: signups_total
    type: metric
    span: 6
    title: Total {events_metric} (Events)
    binding:
      type: duckdb-query
      resources:
        - {EVENTS_PATH}
      sql: |
        SELECT COALESCE(SUM({events_metric}), 0) AS value
        FROM {_parquet_read_sql(EVENTS_PATH)}
      limit: 1
  - id: revenue_total
    type: metric
    span: 6
    title: Total {orders_metric} (Orders)
    binding:
      type: duckdb-query
      resources:
        - {ORDERS_PATH}
      sql: |
        SELECT COALESCE(SUM({orders_metric}), 0) AS value
        FROM {_parquet_read_sql(ORDERS_PATH)}
      limit: 1
"""


def main() -> None:
    orders = inspect_dataset(ORDERS_PATH)
    events = inspect_dataset(EVENTS_PATH)
    print(json.dumps({"inspect": {"orders": orders, "events": events}}, indent=2))

    if not orders.get("metricColumn") or not events.get("metricColumn"):
        raise SystemExit(
            "Could not choose metric columns from schema/profile — "
            "ensure facts Parquet is seeded (nxr prepare-first-look)."
        )

    yaml_text = build_interface_yaml(orders, events)
    payload = lattice.propose_interface(
        INTERFACE_PATH,
        yaml_text,
        summary=f"Create {INTERFACE_PATH} from Orders/Events schema/profile",
        source_type="task",
        resource=TASK,
        warnings=[
            "Demo agent seed — safe to reject if you only wanted to rehearse inspect.",
        ],
    )

    proposal_path = (
        Path(lattice.workspace_root()) / ".lattice" / "proposals" / f"{payload['id']}.json"
    )
    assert proposal_path.is_file(), f"missing proposal at {proposal_path}"
    print(json.dumps({"proposalId": payload["id"], "path": str(proposal_path)}, indent=2))
    print("ok")
