# Resource runtime conformance fixtures

Small on-disk files copied into temporary workspaces by
`crates/lattice-core/tests/resource_runtime_conformance.rs`.

| Fixture | Expected profile | Expected diagnostics |
|---|---|---|
| `bad.json` | `json` | `invalid-json` |
| `bad.yaml` | `yaml` | `invalid-yaml` |
| `fake.pdf` | `pdf` | `magic-mismatch` |
| `minimal.pdf` | `pdf` | none |
| `truncated.pdf` | `pdf` | none (header still recognized) |
| `fake.png` | `image` | `magic-mismatch` |
| `valid.png` | `image` | none |

Traversal and symlink-escape cases are generated in Rust because they need
host-specific layout outside this directory.
