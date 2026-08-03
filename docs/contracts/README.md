# Lattice public contracts

Public contract docs for CLI, MCP, HTTP API, format support, integrations, and
open workspace layouts. Available via:

- MCP resources: `lattice://docs/{topic}`
- MCP tool: `workspace.get_lattice_docs` (`topic` or `list`)
- Web: https://lattice-notes.com/llms.txt and https://lattice-notes.com/open/

| Topic | Resource URI |
| --- | --- |
| Contracts overview | `lattice://docs/index` |
| CLI | `lattice://docs/cli` |
| MCP | `lattice://docs/mcp` |
| HTTP API | `lattice://docs/api` |
| Format support | `lattice://docs/formats` |
| Integrations | `lattice://docs/integrations` |
| Open: workspace | `lattice://docs/open/workspace` |
| Open: page | `lattice://docs/open/page` |
| Open: canvas | `lattice://docs/open/canvas` |
| Open: data | `lattice://docs/open/data` |
| Open: dataset | `lattice://docs/open/dataset` |
| Open: notebook | `lattice://docs/open/notebook` |
| Open: chart | `lattice://docs/open/chart` |
| Open: artifact | `lattice://docs/open/artifact` |
| Open: task | `lattice://docs/open/task` |
| Open: docs-project | `lattice://docs/open/docs-project` |

Source of truth for the private umbrella sync is
`lattice-ecosystem/docs/contracts/`. This tree is the public-client mirror
embedded by `crates/lattice-docs-pack`.
