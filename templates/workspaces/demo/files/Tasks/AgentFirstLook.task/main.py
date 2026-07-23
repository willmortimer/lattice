"""First Look agent seed: inspect Orders/Events datasets → propose CRM interface."""

from __future__ import annotations

import csv
import json
from pathlib import Path

import lattice

TASK = "Tasks/AgentFirstLook.task"
INTERFACE_PATH = "CRM.data/interfaces/AgentDigest.interface.yaml"


def csv_columns(dataset_path: str) -> list[str]:
    handle = lattice.dataset(dataset_path)
    csv_files = [
        path for path in handle.list_source_files() if path.suffix.lower() == ".csv"
    ]
    if not csv_files:
        return []
    with csv_files[0].open(newline="", encoding="utf-8") as handle_file:
        reader = csv.reader(handle_file)
        return next(reader, [])


def profile_summary(path: str) -> dict[str, object]:
    handle = lattice.dataset(path)
    parquet = handle.list_parquet_files()
    return {
        "path": path,
        "columns": csv_columns(path),
        "parquet_partitions": len(parquet),
        "sources": [source.name for source in handle.list_source_files()],
    }


def build_interface_yaml(orders: dict[str, object], events: dict[str, object]) -> str:
    orders_cols = ", ".join(str(column) for column in orders["columns"]) or "unknown"
    events_cols = ", ".join(str(column) for column in events["columns"]) or "unknown"
    return f"""format: lattice-interface
version: 1
name: AgentDigest
title: Agent digest
description: |
  Proposed by {TASK} after inspecting Orders and Events datasets.
  Orders columns: {orders_cols}
  Events columns: {events_cols}
layout:
  columns: 12
components:
  - id: signups_total
    type: metric
    span: 6
    title: Total signups (Events)
    binding:
      type: duckdb-query
      resources:
        - Data/Events.dataset
      sql: |
        SELECT COALESCE(SUM(signups), 0) AS value
        FROM read_parquet('Data/Events.dataset/facts/**/*.parquet', hive_partitioning = true, union_by_name = true)
      limit: 1
  - id: revenue_total
    type: metric
    span: 6
    title: Total revenue (Orders)
    binding:
      type: duckdb-query
      resources:
        - Data/Orders.dataset
      sql: |
        SELECT COALESCE(SUM(revenue), 0) AS value
        FROM read_parquet('Data/Orders.dataset/facts/**/*.parquet', hive_partitioning = true, union_by_name = true)
      limit: 1
"""


def main() -> None:
    orders = profile_summary("Data/Orders.dataset")
    events = profile_summary("Data/Events.dataset")
    print(json.dumps({"inspect": {"orders": orders, "events": events}}, indent=2))

    yaml_text = build_interface_yaml(orders, events)
    payload = lattice.propose_interface(
        INTERFACE_PATH,
        yaml_text,
        summary=f"Create {INTERFACE_PATH} from Orders/Events dataset inspection",
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
