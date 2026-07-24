---
title: Engineering architecture
---

# Engineering architecture

Lattice separates canonical content, semantic mutation, specialized rendering,
and optional long-lived services.

```mermaid
flowchart LR
    Files["Markdown · SQLite · Parquet · Canvas · Jupyter"] --> Core["lattice-runtime"]
    Core --> Command["lattice-commands"]
    Core --> Index["lattice-index"]
    Core --> Data["lattice-data / lattice-duckdb"]
    Command --> Tauri["Tauri handlers"]
    Command --> CLI["lattice CLI"]
    Command --> MCP["latticed MCP"]
    Tauri --> React["React shell"]
    Data --> Specialized["Glide · Perspective · Vega-Lite · MapLibre"]
    MCP --> Agents["External or embedded agents"]
```

The frontend coordinates lifecycle and intent. Rust owns canonical resource
state, validation, storage, commands, search, data orchestration, and capability
enforcement.

Related material:

- [[Docs/Product Overview]]
- [[Research/Architecture]]
- [[Research/Local Runtime]]
- [[Product/Principles]]
