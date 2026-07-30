# lattice-capture-core

Provider-neutral screen capture types for Lattice clipper / screenshot flows.

This crate owns [`CaptureBackend`](src/backend.rs), destination enums, source
handles, and [`CapturedImage`](src/image.rs). Platform capture (ScreenCaptureKit,
AppKit overlays) lives in `lattice-capture-macos`.

## Tests

```sh
cargo test -p lattice-capture-core
```

## Out of scope

- macOS bridge (`lattice-capture-macos`)
- Tauri / desktop shortcut wiring
