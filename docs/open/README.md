# Open formats pack

Stable, public descriptions of Lattice workspace layouts and resource packages.
Intended to be:

- Google-indexed on the docs site (`/open/...`)
- Listed from `llms.txt` for LLM/browser agents
- Embeddable in MCP / agent skills
- Fetchable without reading private umbrella docs

## Rules

1. Keep specs short and structural. Link to deeper public engineering docs when
   needed; do not paste private ADR debate.
2. Prefer examples that are valid on disk.
3. Version breaking layout changes explicitly.
4. Unknown fields in manifests should be preserved by compliant tools.

## Pack index

| Path | Resource |
| --- | --- |
| [workspace/](./workspace/) | Workspace root convention |
| [page/](./page/) | Markdown pages |
| [canvas/](./canvas/) | JSON Canvas + Lattice profile |
| [data/](./data/) | SQLite data app package |
| [dataset/](./dataset/) | Parquet analytical dataset |
| [notebook/](./notebook/) | Jupyter notebook |
| [chart/](./chart/) | Vega-Lite chart |
| [artifact/](./artifact/) | HTML/CSS/JS artifact |
| [task/](./task/) | Task package |
| [docs-project/](./docs-project/) | Docs project folder |

Start from [`llms.txt`](./llms.txt) for a machine-oriented map.
