---
title: Architecture
tags: [diagram]
---

# Architecture

A tiny Mermaid sketch of how Lattice keeps the workspace honest:

```mermaid
flowchart LR
  Files[Workspace files] --> Core[Rust command core]
  Core --> UI[Desktop shell]
  Core --> Index[Search index]
  UI -->|semantic commands| Core
```

Related: [[Product/Vision]] and [[Home]].
