/**
 * Lazy font-pack CSS loaders.
 *
 * Startup only registers the default Lattice pack (Fraunces / Space Grotesk /
 * JetBrains Mono). Alternate packs load when Settings selects them or when
 * the resolved theme references a non-default pack.
 */

const loadedPacks = new Set<string>();

type FontImporter = () => Promise<unknown>;

/** Map font-pack ids (and raw family tokens) to dynamic CSS imports. */
const PACK_LOADERS: Record<string, FontImporter> = {
  lattice: () => Promise.resolve(), // eager via styles.css
  instrument: () =>
    Promise.all([
      import("@fontsource/instrument-serif/400.css"),
      import("@fontsource-variable/instrument-sans/index.css"),
      import("@fontsource-variable/fira-code/index.css"),
    ]),
  teletype: () =>
    Promise.all([
      import("@fontsource-variable/source-serif-4/index.css"),
      import("@fontsource-variable/source-sans-3/index.css"),
      import("@fontsource-variable/source-code-pro/index.css"),
    ]),
  transit: () =>
    Promise.all([
      import("@fontsource-variable/inter-tight/index.css"),
      import("@fontsource-variable/inter/index.css"),
      import("@fontsource-variable/roboto-mono/index.css"),
    ]),
  grove: () =>
    Promise.all([
      import("@fontsource-variable/literata/index.css"),
      import("@fontsource-variable/work-sans/index.css"),
      import("@fontsource-variable/ibm-plex-sans/index.css"),
    ]),
  bulletin: () =>
    Promise.all([
      import("@fontsource-variable/newsreader/index.css"),
      import("@fontsource-variable/public-sans/index.css"),
      import("@fontsource-variable/azeret-mono/index.css"),
    ]),
  almanac: () =>
    Promise.all([
      import("@fontsource-variable/eb-garamond/index.css"),
      import("@fontsource-variable/karla/index.css"),
      import("@fontsource-variable/red-hat-mono/index.css"),
    ]),
  legible: () =>
    Promise.all([
      import("@fontsource-variable/atkinson-hyperlegible-next/index.css"),
      import("@fontsource-variable/atkinson-hyperlegible-mono/index.css"),
    ]),
  apple: () => Promise.resolve(), // system fonts
  draft: () =>
    Promise.all([
      import("@fontsource-variable/petrona/index.css"),
      import("@fontsource-variable/nunito-sans/index.css"),
      import("@fontsource-variable/martian-mono/index.css"),
    ]),
  marquee: () =>
    Promise.all([
      import("@fontsource-variable/bodoni-moda/index.css"),
      import("@fontsource-variable/manrope/index.css"),
      import("@fontsource-variable/geist-mono/index.css"),
    ]),
  atelier: () =>
    Promise.all([
      import("@fontsource-variable/crimson-pro/index.css"),
      import("@fontsource-variable/archivo/index.css"),
      import("@fontsource-variable/fira-code/index.css"),
    ]),
  console: () =>
    Promise.all([
      import("@fontsource-variable/geist/index.css"),
      import("@fontsource-variable/geist-mono/index.css"),
      import("@fontsource-variable/ibm-plex-sans/index.css"),
    ]),
  signal: () =>
    Promise.all([
      import("@fontsource-variable/bricolage-grotesque/index.css"),
      import("@fontsource-variable/inter/index.css"),
      import("@fontsource-variable/jetbrains-mono/index.css"),
    ]),
  foundry: () =>
    Promise.all([
      import("@fontsource-variable/source-serif-4/index.css"),
      import("@fontsource-variable/inter/index.css"),
      import("@fontsource-variable/source-code-pro/index.css"),
    ]),
  meridian: () =>
    Promise.all([
      import("@fontsource-variable/inter/index.css"),
      import("@fontsource-variable/geist-mono/index.css"),
    ]),
  ledger: () =>
    Promise.all([
      import("@fontsource-variable/newsreader/index.css"),
      import("@fontsource-variable/source-sans-3/index.css"),
      import("@fontsource-variable/ibm-plex-sans/index.css"),
    ]),
};

/**
 * Ensure CSS for `fontPackId` is loaded. Idempotent; safe to call on every
 * theme apply. Unknown packs are ignored (CSS variables still apply with
 * fallbacks).
 */
export async function ensureFontPackLoaded(fontPackId: string | null | undefined): Promise<void> {
  const id = fontPackId?.trim() || "lattice";
  if (loadedPacks.has(id)) return;
  const loader = PACK_LOADERS[id];
  if (!loader) {
    loadedPacks.add(id);
    return;
  }
  loadedPacks.add(id);
  try {
    await loader();
  } catch (error) {
    loadedPacks.delete(id);
    console.warn(`Failed to load font pack "${id}":`, error);
  }
}

/** Eager default pack ids already present in the main stylesheet. */
export const EAGER_FONT_PACK_IDS = ["lattice"] as const;

for (const id of EAGER_FONT_PACK_IDS) {
  loadedPacks.add(id);
}
