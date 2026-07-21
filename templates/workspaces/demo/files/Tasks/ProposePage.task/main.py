"""Optional SDK demo: propose a page via the injected lattice package."""

import json
from pathlib import Path

import lattice

root = lattice.workspace_root()
payload = lattice.propose_page(
    "Proposals/FromSdk.task.md",
    "# Proposed by Tasks/ProposePage.task\n\nSDK path for First Look demos.\n",
    summary="Create FromSdk.task page from ProposePage.task",
    source_type="task",
    resource="Tasks/ProposePage.task",
)

proposal_path = Path(root) / ".lattice" / "proposals" / f"{payload['id']}.json"
assert proposal_path.is_file(), f"missing proposal at {proposal_path}"
print(json.dumps({"proposalId": payload["id"], "path": str(proposal_path)}))
print("ok")
