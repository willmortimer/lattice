# ADR 0048: Static artifacts are script-free documents

## Status

Accepted

## Decision

`*.artifact/` packages use a progressive manifest. Version 2 declares one of
`static`, `component`, or `application` profiles. This release executes only
`static`: the host sanitizes HTML, inlines approved package CSS and theme
tokens, injects a restrictive CSP, and mounts the result in a bare sandboxed
iframe. It exposes no postMessage bridge, bindings, network, Tauri API, or
same-origin access.

`lattice-static@1` is a CSS-only semantic vocabulary using `lt-*` classes and
`--lt-*` theme variables. Version 1 packages remain supported as explicitly
labelled legacy interactive components; they do not inherit the static
profile's security claims. Component and application profiles are recognized
but visibly unavailable until their separate runtime and capability models
ship.

Ordinary HTML files receive the same script-free Preview next to Source.

## Consequences

- HTML/CSS is an attractive, inspectable model-native artifact path today.
- Scripts, bindings, WASM, permissions, external CSS and publishing metadata
  require package promotion rather than being silently granted to a file.
- The static assembler is shared by future Deck and static-export consumers.
