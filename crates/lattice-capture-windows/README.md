# lattice-capture-windows

Windows capture adapter for Lattice clipper.

This crate currently ships a **stub** [`CaptureBackend`](../lattice-capture-core)
that returns `CaptureError::Unsupported` for enumerate/screenshot. Real Windows
Graphics Capture (WGC) pixel capture replaces the stub bodies in a follow-up.

## Tests

```sh
cargo test -p lattice-capture-windows
cargo check -p lattice-capture-windows
```

## Out of scope (follow-up)

- Windows Graphics Capture session + frame pool
- Interactive region picker
- Shelf `WDA_EXCLUDEFROMCAPTURE`
