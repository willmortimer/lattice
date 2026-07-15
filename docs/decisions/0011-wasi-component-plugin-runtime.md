# ADR 0011: Prefer WASI components for portable backend plugins

## Status
Accepted

## Context
Loading arbitrary native libraries into the main process creates ABI, crash, portability, and security problems.

## Decision
Use the WebAssembly Component Model, WIT interfaces, and a Wasmtime/WASI host as the preferred backend plugin runtime. UI extensions use declarative components, sandboxed frames/WebViews, or constrained SDKs. User scripts may still run as explicit subprocess tasks.

## Consequences
- Plugins are portable and capability-oriented.
- Host APIs must be intentionally designed and versioned.
- Native integrations remain possible as trusted first-party platform plugins where WASM cannot access required OS APIs.
