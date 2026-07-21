"""Fixture task: import lattice and propose a page under .lattice/proposals/."""

import json
from pathlib import Path

import lattice

root = lattice.workspace_root()
payload = lattice.propose_page(
    "Notes/FromSdk.task.md",
    "# Proposed by ProposePage.task\n",
    summary="Create FromSdk.task page",
    source_type="task",
    resource="ProposePage.task",
)

proposal_path = Path(root) / ".lattice" / "proposals" / f"{payload['id']}.json"
assert proposal_path.is_file(), f"missing proposal at {proposal_path}"
print(json.dumps({"proposalId": payload["id"], "path": str(proposal_path)}))
print("ok")
