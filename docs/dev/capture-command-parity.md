# Capture command-core parity

Checklist for native capture surfaces (macOS ScreenCaptureKit bridge, Tauri
host, React shelf). **Encode, ingest, clipboard, and session history must not
be duplicated in Swift or the web UI** — they flow through Rust command core.

Related: [ADR 0052](../decisions/0052-universal-capture-engine.md),
[ADR 0053](../decisions/0053-lattice-media-asset-model.md),
[capture-smoke.md](./capture-smoke.md).

## Layer ownership

| Concern | Owner | Notes |
| --- | --- | --- |
| ScreenCaptureKit session, display enum, region overlay | `lattice-capture-macos` Swift bridge | Interaction + pixel grab only |
| FFI transport PNG (SCK → C bytes) | Swift `ScreenCaptureSession` | Not clipboard/storage policy |
| `CapturedImage` / `CaptureBackend` types | `lattice-capture-core` | Provider-neutral contracts |
| PNG clipboard + WebP storage renditions | `lattice-capture-core::rendition` | ADR 0053 tier 0 |
| Capture Inbox page + asset transaction | `lattice-handlers::ingest_*` | `CommandEngine` semantic writes |
| Platform clipboard pasteboard | Desktop host (`arboard`) | Host I/O after rendition bytes exist |
| In-session capture shelf history | `apps/desktop/src-tauri/src/capture/shelf.rs` | Ring buffer fed by ingest events |
| Notification open routing | `notification_actions.rs` | Routes to `open-resource`; no ingest |
| Permission query/request UI | React settings + Tauri commands | Delegates to `CapturePermissionProvider` |

## End-to-end flow (⌘⇧2)

```text
Menu / shortcut / tray
  → capture::start_screen_clip (Tauri host)
    → MacOsCaptureBackend::screenshot (Rust)
      → LatticeCaptureBridge (Swift SCK + FFI PNG)
    → lattice_handlers::ingest_captured_image
      → lattice_capture_core::rendition (WebP storage)
      → lattice_handlers::create_inbox_capture (workspace transaction)
    → copy_png_to_clipboard (host arboard)
    → emit capture-ingested + shelf::on_ingested + notification stub
  → React listens for capture-ingested (toast / navigation only)
```

## Parity checklist

- [x] Swift bridge does **not** write workspace files or touch pasteboard.
- [x] Swift bridge does **not** maintain capture history (shelf is Rust-only).
- [x] WebP/PNG rendition policy lives in `lattice-capture-core`, not desktop.
- [x] Inbox ingest uses `lattice_handlers::{ingest_captured_image, ingest_png_capture}`.
- [x] React `screenClip.ts` exports event names only — no ingest logic.
- [x] Capture shelf reads `capture_shelf_snapshot` — does not append history.
- [x] `notification_actions` logs/routes open — does not re-ingest.
- [ ] UNUserNotificationCenter posting (stub only; follow-up).
- [ ] LatticeAsset manifest on disk (R1 stores co-located asset; manifest later).

## Adding a new native surface

1. Capture pixels through `CaptureBackend` (or call `ingest_png_capture` with PNG bytes).
2. Route ingest through `lattice_handlers` — never write Inbox pages from Swift/TS.
3. Copy clipboard from host layer using PNG rendition bytes from `png_bytes_from_capture`.
4. Append shelf history only via `shelf::on_ingested` after successful ingest.
5. Extend this checklist if the surface introduces a new concern.

## Verification

```sh
cargo test -p lattice-capture-core -p lattice-handlers
cargo test -p lattice-desktop --features capture
```

Manual: [capture-smoke.md](./capture-smoke.md).
