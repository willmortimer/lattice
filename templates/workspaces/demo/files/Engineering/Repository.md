---
title: Repository and connected roots
---

# Repository and connected roots

The Lattice source repository is the engineering evidence behind this workspace.
The fixture does not bundle a mutable clone or credentials.

## Recording path

1. Connect the owned Lattice GitHub repository from **Connected roots**.
2. Browse the extracted read-only files inside Lattice.
3. Open recent commits in the repository context.
4. Compare the implementation with `Product/Roadmap.data`.
5. Propose a roadmap or release-note change.
6. Review and approve the semantic proposal.

If the connector is unavailable, use [[Engineering/Architecture]],
`Engineering/Delivery.data`, and `Engineering/Build Status.dataset`. They are
deterministic local resources and make no network claim.

## Connector boundary

Connected repository material is an inspectable read-only extract under
Lattice operational state. The owned Git repository remains authoritative.
Authentication, refresh, and future issue or pull-request APIs are separate
capabilities; the workspace’s synthetic issue and pull-request tables are demo
company records, not a fake live GitHub response.
