/**
 * Native product capture for Remotion (Tauri window via tauri-plugin-playwright).
 *
 * Uses CoreGraphics screenshots + native frame recording (ffmpeg → MP4).
 * CDP attach is Windows/WebView2-only — on macOS this socket bridge is the path.
 *
 * Requires macOS Screen Recording permission for the terminal / Cursor host.
 *
 * Local run:
 *
 *   YC_REMOTION_ASSETS=../../../../apps/yc-remotion/public/product \
 *     pnpm --filter @lattice/desktop test:capture:tauri
 *
 * Or two terminals:
 *
 *   pnpm --filter @lattice/desktop tauri:dev:e2e
 *   YC_REMOTION_ASSETS=... LATTICE_PERF_REUSE_TAURI=1 \
 *     pnpm --filter @lattice/desktop test:capture:tauri
 */
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { openTreePage, waitForShellChrome } from "../perf/helpers";
import { expect, test } from "../fixtures";

const __dirname = dirname(fileURLToPath(import.meta.url));

const DEFAULT_OUT = resolve(
  __dirname,
  "../../../../../apps/yc-remotion/public/product",
);

type Beat = {
  id: string;
  label: string;
  settleMs: number;
  holdMs: number;
};

const BEATS: Beat[] = [
  { id: "home", label: "Page: Home.md", settleMs: 800, holdMs: 2200 },
  {
    id: "roadmap",
    label: "Data app: Product/Roadmap.data",
    settleMs: 1200,
    holdMs: 2400,
  },
  {
    id: "orders",
    label: "Dataset: Data/Orders.dataset",
    settleMs: 1800,
    holdMs: 2400,
  },
  {
    id: "chart",
    label: "File: Dashboards/Revenue by day.vl.json",
    settleMs: 1600,
    holdMs: 2400,
  },
  {
    id: "canvas",
    label: "Canvas: Canvases/Product Strategy.canvas",
    settleMs: 1400,
    holdMs: 2400,
  },
  { id: "crm", label: "Data app: CRM.data", settleMs: 1200, holdMs: 2200 },
];

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

test.describe("YC product capture (tauri native)", () => {
  test.setTimeout(240_000);

  test("captures stills and short clips of First Look beats", async ({
    tauriPage,
  }) => {
    const outRoot = resolve(process.env.YC_REMOTION_ASSETS ?? DEFAULT_OUT);
    const stillsDir = resolve(outRoot, "stills");
    const clipsDir = resolve(outRoot, "clips");
    await mkdir(stillsDir, { recursive: true });
    await mkdir(clipsDir, { recursive: true });

    tauriPage.setDefaultTimeout(45_000);
    await waitForShellChrome(tauriPage);

    const manifestBeats: Array<{
      id: string;
      label: string;
      still: string;
      clip: string | null;
    }> = [];

    for (const beat of BEATS) {
      const clipDir = resolve(clipsDir, `_raw-${beat.id}`);
      await tauriPage.startRecording({ path: clipDir, fps: 15 });

      await openTreePage(tauriPage, beat.label);
      await sleep(beat.settleMs);

      const stillPath = resolve(stillsDir, `${beat.id}.png`);
      const png = await tauriPage.screenshot({ path: stillPath });
      expect(png.length, `screenshot for ${beat.id} should be non-empty`).toBeGreaterThan(
        1_000,
      );

      await sleep(beat.holdMs);

      const recording = await tauriPage.stopRecording();
      const clipRel = recording.video
        ? `product/clips/${beat.id}.mp4`
        : null;

      if (recording.video) {
        const { rename } = await import("node:fs/promises");
        await rename(recording.video, resolve(clipsDir, `${beat.id}.mp4`));
      }

      manifestBeats.push({
        id: beat.id,
        label: beat.label,
        still: `product/stills/${beat.id}.png`,
        clip: clipRel,
      });
    }

    const manifest = {
      capturedAt: new Date().toISOString(),
      mode: "tauri-native",
      outRoot,
      beats: manifestBeats,
      note: "Native CoreGraphics capture via tauri-plugin-playwright. CDP mode is Windows/WebView2-only.",
    };
    await writeFile(
      resolve(outRoot, "manifest.json"),
      `${JSON.stringify(manifest, null, 2)}\n`,
    );
  });
});
