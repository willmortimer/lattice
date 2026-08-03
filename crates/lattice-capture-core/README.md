# lattice-capture-core

Provider-neutral screen capture types for Lattice clipper / screenshot flows.

This crate owns [`CaptureBackend`](src/backend.rs), destination enums, source
handles, and [`CapturedImage`](src/image.rs). Platform capture lives in
`lattice-capture-macos` (ScreenCaptureKit) and `lattice-capture-windows`
(WGC stub until real capture lands).

## Tests

```sh
cargo test -p lattice-capture-core
```

## Out of scope

- macOS bridge (`lattice-capture-macos`)
- Windows Graphics Capture (`lattice-capture-windows`)
- Tauri / desktop shortcut wiring
