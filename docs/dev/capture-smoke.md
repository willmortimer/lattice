# Screen capture smoke (R1)

Manual verification for macOS still capture (ScreenCaptureKit → clipboard +
Capture Inbox). Automated tests cover path helpers, ingest validation, and event
constants only — **CI does not run interactive ScreenCaptureKit overlays**.

Related: [capture and media roadmap §R1](../architecture/capture-and-media.md).

## Build

From the `lattice` repo root (macOS only):

```sh
# Nix / NXR (preferred)
nxr desktop-dev -- --features capture

# Or pnpm from apps/desktop
pnpm --filter @lattice/desktop exec tauri dev --features capture
```

The `capture` Cargo feature links `lattice-capture-macos` and registers the
global shortcut. Linux CI builds stay featureless and skip this path.

## Prerequisites

1. Open **System Settings → Privacy & Security → Screen Recording**.
2. Enable **Lattice** (or the dev `lattice-desktop` binary).
3. Restart the app if permission was just granted.

## Happy path

1. Focus any app (Lattice may be in the background).
2. Press **⌘⇧2** or choose **Screen Clip** from the app menu / tray.
3. If the interactive region overlay appears, drag a rectangle and confirm
   (or dismiss to test cancel — see below).
4. Expect:
   - **Clipboard:** PNG image (paste into Preview or another app).
   - **Workspace:** new page under the configured quick-note directory
     (default `Inbox/`) titled **Screen clip** with an embedded asset.
   - **Asset file:** `assets/capture.webp` (lossless WebP) or `assets/capture.png`
     if WebP encode fails.
5. In the Lattice UI, open the new inbox page and confirm the image renders.

## Cancel / interactive stub

1. Trigger **⌘⇧2** / **Screen Clip**.
2. Press **Esc** or click outside the overlay without confirming a region.
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

## Automated checks (no Screen Recording)

```sh
cargo test -p lattice-capture-core
cargo test -p lattice-handlers capture::
pnpm --filter @lattice/desktop test src/screenClip.test.ts
```

These tests validate destination/source/plan types, inbox ingest limits,
filename sanitization, collision renames, and stable event constants. They do
**not** invoke ScreenCaptureKit or require Screen Recording permission.
