# EDR / HDR and wide-gamut color in the theme engine

Status: feasibility report (2026-07). No engine changes shipped for HDR yet;
this documents what already works, what breaks, and what a real
implementation would look like.

## Terms

- **Wide gamut (Display P3)** — more saturated primaries than sRGB at the
  same brightness. Every Apple Silicon Mac, iPhone, and most modern panels
  cover P3. This is a *color space* question.
- **EDR (Extended Dynamic Range)** — Apple's compositor model where color
  component values above 1.0 map to brightness headroom above SDR white
  (`NSScreen.maximumExtendedDynamicRangeColorComponentValue`). This is a
  *brightness* question.
- **HDR (HDR10/Dolby Vision/HLG)** — transfer-function-encoded high dynamic
  range, mostly a video/image concern on the web today.

## What works today, unchanged

Theme role values are free-form CSS color strings — the Rust validator and
`scripts/compile-theme.mjs` pass them through untouched. A user theme can
already write:

```yaml
palette:
  accent: "color(display-p3 1 0.62 0.11)"
```

and the flattened `--lt-accent` lands in CSS intact. WKWebView (Tauri's
macOS webview) has supported `color(display-p3 …)` since Safari 15 and
color-manages it correctly on P3 panels. Our derived washes use
`color-mix(in oklch, …)`, and OKLCH interpolation is gamut-agnostic, so
mixes of P3 inputs stay correct.

## What breaks if a theme uses P3/HDR colors

1. **The Pixi/canvas mirror** (`theme-tokens.ts`): `hexToRgba()` in
   `compile-theme.mjs` requires `#RRGGBB` and will throw. Canvas contexts
   are also created in sRGB by default; P3 values would be clamped unless
   the context is created with `{ colorSpace: "display-p3" }` (supported in
   WebKit and Chromium).
2. **The terminal** (`terminalTheme.ts`): xterm.js parses only
   hex/`rgb()`-style colors. A `color(display-p3 …)` string in a `--lt-term-*`
   or role token silently falls back to xterm defaults. (This is also why
   `--lt-accent-wash` — a `color-mix()` expression — already falls back for
   `selectionBackground`; the new `terminal:` blocks sidestep that by
   providing literal selection colors.)
3. **Native window background**: `applyResolvedTheme` passes
   `resolved.background` to Tauri's `setBackgroundColor`, which expects a
   parseable sRGB color.

So: shipped built-ins stay `#RRGGBB` on purpose.

## True EDR/HDR (brightness > SDR white): not reachable from CSS yet

- WKWebView composites CSS content in SDR. EDR headroom is only exposed to
  web content for HDR *video* (and image formats like gain-map HEIC/AVIF) —
  not for CSS colors or canvas fills, as of Safari 26.
- The CSS Color HDR spec (`dynamic-range-limit`, HDR color spaces like
  `rec2100-pq`) is still a working draft; Chromium has experimental
  support, WebKit does not expose it to apps yet.
- The `@media (dynamic-range: high)` / `(color-gamut: p3)` queries *do*
  work in WKWebView and are the right progressive-enhancement hooks.
- A native escape hatch exists (a `CAMetalLayer` with
  `wantsExtendedDynamicRangeContent` behind the webview), but that bypasses
  the DOM entirely — not worth it for UI chrome.

## Recommended adoption path (when we want it)

1. **Schema**: allow an optional per-color P3 variant, e.g.
   `accent_p3: "color(display-p3 …)"`, or a theme-level
   `color_profile: p3` flag. Keep the sRGB value required — it is the
   fallback and the only value the terminal/canvas mirrors consume.
2. **Compiler**: emit P3 overrides inside
   `@media (color-gamut: p3) { :root { --lt-accent: … } }` so sRGB panels
   and the canvas/terminal keep the hex value.
3. **Canvas**: create Pixi/2D contexts with `colorSpace: "display-p3"` when
   `matchMedia("(color-gamut: p3)")` matches, and extend `hexToRgba` to
   pass P3 strings through.
4. **EDR**: revisit once WebKit ships CSS Color HDR; the engine's
   flatten-to-vars design means it would be another media-gated var block,
   no schema change required.

Net: **P3 wide gamut is practical now** with a small, backward-compatible
schema addition; **true EDR brightness is blocked on WebKit**, and nothing
in the theme engine's design would need to change when it lands.
