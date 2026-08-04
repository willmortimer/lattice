# Screen capture smoke (R1)

Manual verification for still capture (clipboard + Capture Inbox). macOS uses
ScreenCaptureKit; Windows uses Windows Graphics Capture (WGC). Automated tests
cover path helpers, ingest validation, and event constants only — **CI does not
run interactive capture overlays**.

Related: [capture and media roadmap §R1](../architecture/capture-and-media.md).

## Build

### macOS

From the `lattice` repo root:

```sh
# Nix / NXR (preferred)
nxr desktop-dev -- --features capture

# Or pnpm from apps/desktop
pnpm --filter @lattice/desktop exec tauri dev --features capture
```

### Windows

Dev (unsigned):

```sh
pnpm --filter @lattice/desktop exec tauri dev --features capture
```

Release / NSIS (unsigned beta chain):

```sh
pnpm --filter @lattice/desktop run tauri:build:windows
# or winbuild:
WINBUILD_TASKS='probe ensure-toolchain build-sidecar verify-sidecars tauri-bundle' \
  ./scripts/winbuild/remote-windows-check.sh
```

Windows NSIS / `tauri-bundle.ps1` pass `--features capture` (no `voice-embedded`).

The `capture` Cargo feature links platform backends (`lattice-capture-macos` or
`lattice-capture-windows`) and registers the global shortcut. Linux CI builds
stay featureless and skip this path.

## Prerequisites

### macOS

1. Open **System Settings → Privacy & Security → Screen Recording**.
2. Enable **Lattice** (or the dev `lattice-desktop` binary).
3. Restart the app if permission was just granted.

### Windows

1. Windows 10 1903+ or Windows 11 with WGC support.
2. If capture fails, open **Settings → Privacy & security → Graphics capture**
   (or run Settings from the app if a permission command is exposed) and ensure
   desktop apps may capture.
3. Open a workspace in Lattice before clipping (capture requires a workspace root).

## Happy path

1. Focus any app (Lattice may be in the background).
2. Press **Ctrl+Shift+2** (Windows) or **⌘⇧2** (macOS), or choose **Screen Clip**
   from the app menu / tray.
3. If the interactive region overlay appears, drag a rectangle and confirm
   (or dismiss to test cancel — see below).
4. Expect:
   - **Clipboard:** PNG image (paste into Preview, Paint, or another app).
   - **Workspace:** new page under the configured quick-note directory
     (default `Inbox/`) titled **Screen clip** with an embedded asset.
   - **Asset file:** `assets/capture.webp` (lossless WebP) or `assets/capture.png`
     if WebP encode fails.
5. In the Lattice UI, open the new inbox page and confirm the image renders.

## Shelf exclusion (Windows chrome)

Lattice chrome must not appear in clipped pixels.

1. Leave the capture shelf open (or trigger a clip so it floats after ingest).
2. Keep the main Lattice window visible on the display you are clipping.
3. Press **Ctrl+Shift+2** and capture a region that would include the shelf
   and/or main window chrome.
4. Expect: the PNG clipboard image (and inbox asset) shows the underlying
   desktop/apps only — **no** Lattice shelf or main window chrome.
5. Passive that the shelf floated after ingest without stealing focus from the
   app you were using before the clip.

macOS exclusion continues to use ScreenCaptureKit content filters; this
checklist is the Windows `WDA_EXCLUDEFROMCAPTURE` always-on verification.

## Cancel / interactive overlay

1. Trigger the shortcut or **Screen Clip**.
2. Press **Esc** or click outside the overlay without confirming a region
   (Windows: right-click also cancels).
3. Expect:
   - No new inbox page.
   - No clipboard change.
   - Frontend may receive `capture-cancelled` (see `screenClip.ts`).

When the interactive overlay is unavailable (unsupported backend), capture falls
back to the primary display without user interaction.

## Multi-monitor

1. Connect an external display.
2. Run capture and select a region on the non-primary monitor when the overlay
   is available.
3. Confirm the clipped pixels match the chosen region (not a cropped primary
   display).

## Automated checks (no Screen Recording / WGC)

```sh
cargo test -p lattice-capture-core
cargo test -p lattice-capture-windows
cargo test -p lattice-handlers capture::
cargo check -p lattice-desktop --features capture
pnpm --filter @lattice/desktop test src/screenClip.test.ts
```

These tests validate destination/source/plan types, inbox ingest limits,
filename sanitization, collision renames, and stable event constants. They do
**not** invoke ScreenCaptureKit or WGC overlays or require capture permission.
