# Data Support Polish DAG

**Status:** Active  
**Created:** 2026-07-20  
**BASE:** `main`  
**Integration branch:** `feat/data-support-polish`  
**Models:** `composer-2.5` (routine) · `cursor-grok-4.5-high` (P2P06 harness)

See Cursor plan `data_support_polish` for full handoff packets. Waves:

1. A — P2P01 ‖ P2P03 ‖ P2P04 ‖ P2P05 ‖ P2P08  
2. B — P2P02 (after P01)  
3. C — P2P06 (after P03+P04+P05)  
4. D — P2P07 (after P01+P04+P06+P08)

## Polish status

- **P2P08** (field-type docs alignment): done — `docs/10` § Typed fields lists
  only shipped `FieldType` values (`text`, `long_text`, `integer`, `decimal`,
  `boolean`, `date`, `relation`, `lookup`, `rollup`); formula and attachment
  **columns** are Phase 2+ roadmap; workspace `attachmentsDirectory` clarified
  as unrelated.
