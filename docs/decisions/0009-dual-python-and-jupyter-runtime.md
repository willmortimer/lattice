# ADR 0009: Use Pyodide for instant execution and native Jupyter for serious compute

## Status
Accepted

## Context
Pyodide provides portable browser Python but has package, process, performance, and networking limitations. Native Python offers compatibility but requires environment management. Jupyter provides a language-independent execution protocol.

## Decision
Support:
- Pyodide workers for zero-setup sandboxed snippets and browser notebooks;
- native Python subprocesses managed with `uv`;
- first-class Jupyter kernels for Python, R, Julia, SQL, and remote compute;
- optional Nix, container, WASI, and remote execution providers.

Do not embed CPython directly into the trusted desktop process initially.

## Consequences
- Lightweight and serious workflows both have appropriate paths.
- Runtime manifests and capability policies are required.
- Notebook files remain standard `.ipynb` resources with optional Lattice sidecars.
