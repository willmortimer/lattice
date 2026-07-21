"""File-based transaction proposals under ``.lattice/proposals/``.

JSON matches Rust ``TransactionProposal`` serde (camelCase fields;
``Command`` tagged with ``type`` in kebab-case). Writes are side-effect only —
they do not call the CommandEngine.
"""

from __future__ import annotations

import json
import re
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from lattice._env import workspace_root

OPERATIONAL_DIR = ".lattice"
PROPOSALS_DIR = "proposals"

_SOURCE_TYPES = frozenset({"task", "workflow", "artifact", "mcp", "external"})


def _now_iso() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _new_proposal_id() -> str:
    return str(uuid.uuid4())


def _normalize_rel_path(path: str) -> str:
    text = path.strip().replace("\\", "/")
    while text.startswith("./"):
        text = text[2:]
    if not text or text.startswith("/") or re.search(r"(^|/)\.\.(/|$)", text):
        raise ValueError(f"proposal path must be workspace-relative: {path!r}")
    return text


def _proposals_dir(root: Path) -> Path:
    return root / OPERATIONAL_DIR / PROPOSALS_DIR


def _validate_source(source_type: str) -> str:
    normalized = source_type.strip().lower()
    if normalized not in _SOURCE_TYPES:
        allowed = ", ".join(sorted(_SOURCE_TYPES))
        raise ValueError(f"unknown proposal source type {source_type!r}; expected one of: {allowed}")
    return normalized


def _affected_paths(commands: list[dict[str, Any]]) -> list[str]:
    paths: list[str] = []
    seen: set[str] = set()
    for command in commands:
        path = command.get("path")
        if isinstance(path, str):
            rel = _normalize_rel_path(path)
            if rel not in seen:
                seen.add(rel)
                paths.append(rel)
    return paths


def propose(
    *,
    commands: list[dict],
    summary: str,
    source_type: str = "task",
    resource: str | None = None,
    warnings: list[str] | None = None,
    proposal_id: str | None = None,
    created_at: str | None = None,
) -> dict:
    """Write a pending ``TransactionProposal`` JSON file and return its payload."""
    if not commands:
        raise ValueError("propose() requires at least one command")
    if not summary or not summary.strip():
        raise ValueError("propose() requires a non-empty summary")

    root = workspace_root()
    source: dict[str, Any] = {"type": _validate_source(source_type)}
    if resource is not None and resource.strip():
        source["resource"] = resource.strip()

    normalized_commands: list[dict[str, Any]] = []
    for command in commands:
        if not isinstance(command, dict) or "type" not in command:
            raise ValueError(f"command must be an object with a type field: {command!r}")
        entry = dict(command)
        if "path" in entry and isinstance(entry["path"], str):
            entry["path"] = _normalize_rel_path(entry["path"])
        normalized_commands.append(entry)

    payload: dict[str, Any] = {
        "id": proposal_id or _new_proposal_id(),
        "source": source,
        "summary": summary.strip(),
        "commands": normalized_commands,
        "affectedPaths": _affected_paths(normalized_commands),
        "warnings": list(warnings or []),
        "createdAt": created_at or _now_iso(),
    }

    out_dir = _proposals_dir(root)
    out_dir.mkdir(parents=True, exist_ok=True)
    out_path = out_dir / f"{payload['id']}.json"
    out_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    return payload


def propose_page(
    path: str,
    content: str,
    *,
    summary: str | None = None,
    source_type: str = "task",
    resource: str | None = None,
    warnings: list[str] | None = None,
) -> dict:
    """Propose a ``page-create`` command for ``path`` with Markdown ``content``."""
    rel = _normalize_rel_path(path)
    return propose(
        commands=[{"type": "page-create", "path": rel, "content": content}],
        summary=summary or f"Create page {rel}",
        source_type=source_type,
        resource=resource,
        warnings=warnings,
    )
