# Phase 3 Polish + Data MVP DAG

**Status:** Active  
**Created:** 2026-07-20  
**BASE:** `origin/main`  
**Integration branch:** `feat/phase3-polish`  
**Models:** `cursor-grok-4.5-high` only (`best-of-n-runner` worktrees)

See Cursor plan `phase3_polish_dag` for full handoff packets.

## Locked MVP cuts

- Formula: `FieldType::Formula` + friendly expressions; read-time only; no SQL layer
- Junction: internal SQLite M2M for one First Look demo pair; JSON TEXT `Relation` UX
- Cross-package: read-only picker/labels; no write-through
- Geo: lon/lat GeoParquet + MapLibre; no DuckDB spatial / deck.gl

## Waves

1. A — P3P01 ‖ P3P02 ‖ P3P05 ‖ P2S01 ‖ P2F01  
2. B — P3P03 then P3P06 ‖ P2J01  
3. C — P3P04 ‖ P2X01 ‖ P2S02  
4. D — P3P07 docs + gate

## Packet status

| ID | Title | Status |
| --- | --- | --- |
| P3P01 | DuckDB EXPLAIN + Tauri | pending |
| P3P02 | Cancel backend | pending |
| P3P03 | Plan tab UI | pending |
| P3P04 | Cancel frontend | pending |
| P3P05 | Places GeoParquet seed | pending |
| P3P06 | MapLibre Map tab | pending |
| P2S01 | Native tree/undo smoke | pending |
| P2F01 | Formula MVP | pending |
| P2J01 | Junction M2M demo | pending |
| P2X01 | Cross-package read-only | pending |
| P2S02 | Native schema/import smoke | pending |
| P3P07 | Docs + roadmap closeout | pending |
