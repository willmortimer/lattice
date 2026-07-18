---
title: Principles
tags: [product]
---

# Principles

Guiding constraints for Lattice — referenced from [[Product/Vision]] and [[Product/Roadmap]].

## Local-first

The workspace is a directory on disk. Canonical content stays inspectable outside
Lattice. Offline is the normal state.

## Commands, not side doors

Every mutation flows through the semantic command core. The React shell coordinates;
it does not become a privileged writer.

## Progressive disclosure

Primary creation vocabulary: **Page**, **Canvas**, **Table**, **Notebook**, **File**.
Advanced source, history, and conflicts belong under per-resource Inspect surfaces.

## Honest reconciliation

External edits are legitimate. Lattice watches the tree and reconciles without
silent data loss.

## Performance as product

Large pages like [[Research/Long Read]], wide tables in `CRM.data`, and canvas
panning should meet documented budgets before new abstractions land.

See [[Home]] for a tour of this sample workspace.
