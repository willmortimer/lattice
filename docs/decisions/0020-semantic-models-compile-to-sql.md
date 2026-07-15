# ADR 0020: Analytical semantic models compile to open query primitives

## Status
Accepted

## Context
Tableau and Power BI demonstrate the value of measures, dimensions, relationships, hierarchies, and live/extract/composite modes, but proprietary languages such as DAX would undermine Lattice's open-native premise.

## Decision
Define readable semantic-model manifests whose measures and relationships compile to SQL, DuckDB plans, or connector-supported operations. Use Python, R, Julia, or notebook code for computations that do not fit relational semantics. Do not create a proprietary general-purpose query language.

## Consequences
- Models remain inspectable and engine-compatible.
- Some advanced BI semantics require clear compilation rules.
- Substrait may be used internally as a portable plan representation.
