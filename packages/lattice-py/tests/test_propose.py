"""Unit tests for proposal JSON shape (matches Rust TransactionProposal)."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

import lattice


@pytest.fixture()
def workspace(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    monkeypatch.setenv("LATTICE_WORKSPACE", str(tmp_path))
    return tmp_path


def test_propose_page_writes_camelcase_transaction_proposal(workspace: Path) -> None:
    payload = lattice.propose_page(
        "Notes/FromTask.md",
        "# Hello from task\n",
        summary="Create FromTask page",
        source_type="task",
        resource="Tasks/Demo.task",
    )

    assert payload["id"]
    assert payload["source"] == {"type": "task", "resource": "Tasks/Demo.task"}
    assert payload["summary"] == "Create FromTask page"
    assert payload["commands"] == [
        {
            "type": "page-create",
            "path": "Notes/FromTask.md",
            "content": "# Hello from task\n",
        }
    ]
    assert payload["affectedPaths"] == ["Notes/FromTask.md"]
    assert payload["warnings"] == []
    assert "T" in payload["createdAt"] and payload["createdAt"].endswith("Z")
    assert "status" not in payload

    on_disk = workspace / ".lattice" / "proposals" / f"{payload['id']}.json"
    assert on_disk.is_file()
    loaded = json.loads(on_disk.read_text(encoding="utf-8"))
    assert loaded == payload


def test_propose_accepts_external_source(workspace: Path) -> None:
    payload = lattice.propose(
        commands=[{"type": "page-create", "path": "A.md", "content": "x"}],
        summary="External note",
        source_type="external",
    )
    assert payload["source"] == {"type": "external"}
    assert (workspace / ".lattice" / "proposals" / f"{payload['id']}.json").is_file()


def test_workspace_helper_dataset_and_propose(workspace: Path) -> None:
    ds_dir = workspace / "Data" / "Orders.dataset"
    ds_dir.mkdir(parents=True)
    (ds_dir / "sources").mkdir()
    (ds_dir / "sources" / "orders.csv").write_text("id,qty\n1,2\n", encoding="utf-8")

    handle = lattice.workspace.dataset("Data/Orders.dataset")
    assert handle.path == "Data/Orders.dataset"
    assert handle.absolute_path == ds_dir.resolve()
    assert handle.exists()
    assert handle.list_source_files()[0].name == "orders.csv"

    payload = lattice.workspace.propose_page("Notes/ViaHelper.md", "body")
    assert payload["commands"][0]["type"] == "page-create"


def test_workspace_root_requires_env(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("LATTICE_WORKSPACE", raising=False)
    with pytest.raises(lattice.LatticeWorkspaceError):
        lattice.workspace_root()


def test_dataset_rejects_escape(workspace: Path) -> None:
    with pytest.raises(ValueError):
        lattice.dataset("../outside.dataset")


def test_sample_fixture_json_shape() -> None:
    """Golden sample checked by Rust ``TransactionProposal`` deserialize test."""
    sample = Path(__file__).resolve().parents[1] / "testdata" / "sample_proposal.json"
    payload = json.loads(sample.read_text(encoding="utf-8"))
    assert payload["source"]["type"] == "task"
    assert payload["commands"][0]["type"] == "page-create"
    assert payload["affectedPaths"] == ["Notes/SdkSample.md"]
    assert "status" not in payload
