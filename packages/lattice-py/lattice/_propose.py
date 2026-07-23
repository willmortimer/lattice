"""File-based transaction proposals under ``.lattice/proposals/``.

JSON matches Rust ``TransactionProposal`` serde (camelCase fields;
``Command`` tagged with ``type`` in kebab-case). Writes are side-effect only —
they do not call the CommandEngine.
"""

from __future__ import annotations

import base64
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


def _resource_create_command(path: str, content: str) -> dict[str, Any]:
    rel = _normalize_rel_path(path)
    encoded = base64.b64encode(content.encode("utf-8")).decode("ascii")
    return {"type": "resource-create", "path": rel, "content": encoded}


def propose_resource(
    path: str,
    content: str,
    *,
    summary: str | None = None,
    source_type: str = "task",
    resource: str | None = None,
    warnings: list[str] | None = None,
) -> dict:
    """Propose a ``resource-create`` command for text ``content`` at ``path``."""
    command = _resource_create_command(path, content)
    rel = command["path"]
    assert isinstance(rel, str)
    return propose(
        commands=[command],
        summary=summary or f"Create resource {rel}",
        source_type=source_type,
        resource=resource,
        warnings=warnings,
    )


def propose_workflow(
    path: str,
    content: str,
    *,
    summary: str | None = None,
    source_type: str = "task",
    resource: str | None = None,
    warnings: list[str] | None = None,
) -> dict:
    """Propose creating a workflow YAML file (caller should validate YAML offline if needed)."""
    rel = _normalize_rel_path(path)
    extra = list(warnings or [])
    if not rel.lower().endswith(".workflow.yaml"):
        extra.append(
            f"path {rel!r} does not end with .workflow.yaml; workflow discovery may ignore it"
        )
    return propose(
        commands=[_resource_create_command(rel, content)],
        summary=summary or f"Create workflow {rel}",
        source_type=source_type,
        resource=resource,
        warnings=extra,
    )


def propose_interface(
    path: str,
    content: str,
    *,
    summary: str | None = None,
    source_type: str = "task",
    resource: str | None = None,
    warnings: list[str] | None = None,
) -> dict:
    """Propose creating an interface YAML file."""
    rel = _normalize_rel_path(path)
    extra = list(warnings or [])
    if not rel.lower().endswith(".interface.yaml"):
        extra.append(
            f"path {rel!r} does not end with .interface.yaml; package loaders may ignore it"
        )
    return propose(
        commands=[_resource_create_command(rel, content)],
        summary=summary or f"Create interface {rel}",
        source_type=source_type,
        resource=resource,
        warnings=extra,
    )


def propose_artifact(
    path: str,
    content: str,
    *,
    summary: str | None = None,
    source_type: str = "task",
    resource: str | None = None,
    warnings: list[str] | None = None,
) -> dict:
    """Propose creating an artifact.yaml manifest (package path or manifest path)."""
    rel = _normalize_rel_path(path)
    lower = rel.lower()
    if lower.endswith("artifact.yaml"):
        manifest = rel
    elif lower.endswith(".artifact"):
        manifest = f"{rel.rstrip('/')}/artifact.yaml"
    else:
        raise ValueError("artifact path must end with .artifact or artifact.yaml")
    extra = list(warnings or [])
    extra.append(
        "proposal writes artifact.yaml only; entrypoint HTML and package dirs still need separate commands"
    )
    return propose(
        commands=[_resource_create_command(manifest, content)],
        summary=summary or f"Create artifact manifest {manifest}",
        source_type=source_type,
        resource=resource,
        warnings=extra,
    )
