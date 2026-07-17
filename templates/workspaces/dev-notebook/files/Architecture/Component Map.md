---
title: Component Map
---

# Component map

High-level request flow for a typical Lattice feature.

```mermaid
flowchart LR
  subgraph shell [Desktop shell]
    UI[React shell]
    IPC[Tauri IPC]
  end
  subgraph core [Rust core]
    CMD[Command layer]
    DOM[Domain services]
    IDX[Search index]
  end
  FS[(Workspace files)]

  UI --> IPC --> CMD --> DOM --> FS
  DOM --> IDX
```

## Related

- Spatial layout: [[Architecture/System Overview.canvas]]
- Decision log: [[Decisions/0001-record-architecture-decisions]]
- Open issues: `Issues.data`
