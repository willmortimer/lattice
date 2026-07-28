/**
 * Native product capture for Remotion (Tauri window via tauri-plugin-playwright).
 *
 * Uses CoreGraphics screenshots + native frame recording (ffmpeg → MP4).
 * CDP attach is Windows/WebView2-only — on macOS this socket bridge is the path.
 *
 * When `LATTICE_DEV_DEMO_OVERLAY` seeded the private hackathon pitch package,
 * an extra Pitch.deck beat is captured. Capture-only cursor + callout chrome is
 * injected via `demoDirector.ts` (not permanent product UI).
 *
 * Preferred from the ecosystem umbrella:
 *
 *   ./scripts/exec-for-dev.sh --capture-yc
 *
 * Or:
 *
 *   YC_REMOTION_ASSETS=../../../../apps/yc-remotion/public/product \
 *     pnpm --filter @lattice/desktop test:capture:tauri
 */
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { waitForShellChrome } from "../perf/helpers";
import { expect, test } from "../fixtures";
import {
  clearCaptureDirector,
  hideCaptureCallouts,
  hideCaptureCursor,
  installCaptureDirector,
  scriptedOpenTreePage,
  waitForOverlayTreeLabel,
  type CaptureCallout,
} from "./demoDirector";

const __dirname = dirname(fileURLToPath(import.meta.url));

const DEFAULT_OUT = resolve(
  __dirname,
  "../../../../../apps/yc-remotion/public/product",
);

const PITCH_DECK_LABEL = "Deck: Hackathon/Pitch.deck";

type Beat = {
  id: string;
  label: string;
  settleMs: number;
  holdMs: number;
  callout?: CaptureCallout;
};

const CORE_BEATS: Beat[] = [
  {
    id: "home",
    label: "Page: Home.md",
    settleMs: 800,
    holdMs: 2200,
    callout: {
      title: "Your folder is the workspace",
      body: "Pages, tables, and notebooks stay ordinary files on disk.",
      placement: "right",
    },
  },
  {
    id: "roadmap",
    label: "Data app: Product/Roadmap.data",
    settleMs: 1200,
    holdMs: 2400,
    callout: {
      title: "Product and ops together",
      body: "Roadmaps and status without a separate SaaS silo.",
      placement: "right",
    },
  },
  {
    id: "orders",
    label: "Dataset: Data/Orders.dataset",
    settleMs: 1800,
    holdMs: 2400,
    callout: {
      title: "Real data, not a paste",
      body: "Open Preview, Chart, and Profile on the same package.",
      placement: "right",
    },
  },
  {
    id: "chart",
    label: "File: Dashboards/Revenue by day.vl.json",
    settleMs: 1600,
    holdMs: 2400,
    callout: {
      title: "Dashboards that point at truth",
      body: "Charts reference live workspace resources.",
      placement: "left",
    },
  },
  {
    id: "canvas",
    label: "Canvas: Canvases/Product Strategy.canvas",
    settleMs: 1400,
    holdMs: 2400,
    callout: {
      title: "Spatial thinking on a canvas",
      body: "Arrange the story without leaving the files.",
      placement: "right",
    },
  },
  {
    id: "crm",
    label: "Data app: CRM.data",
    settleMs: 1200,
    holdMs: 2200,
    callout: {
      title: "Tables that grow into apps",
      body: "Forms, views, and workflows stay on disk.",
      placement: "right",
    },
  },
];

const PITCH_BEAT: Beat = {
  id: "pitch",
  label: PITCH_DECK_LABEL,
  settleMs: 1600,
  holdMs: 2600,
  callout: {
    title: "Presenter-native close",
    body: "Private overlay: Hackathon/Pitch.deck — not shipped in public First Look.",
    placement: "left",
  },
};

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

test.describe("YC product capture (tauri native)", () => {
  test.setTimeout(300_000);

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
    await installCaptureDirector(tauriPage);

    const overlayEnabled = Boolean(process.env.LATTICE_DEV_DEMO_OVERLAY?.trim());
    let beats = [...CORE_BEATS];
    if (overlayEnabled) {
      const ready = await waitForOverlayTreeLabel(tauriPage, PITCH_DECK_LABEL);
      if (ready) {
        beats = [...CORE_BEATS, PITCH_BEAT];
      } else {
        console.warn(
          `capture: LATTICE_DEV_DEMO_OVERLAY set but ${PITCH_DECK_LABEL} not in tree; skipping pitch beat`,
        );
      }
    }

    const manifestBeats: Array<{
      id: string;
      label: string;
      still: string;
      clip: string | null;
      callout?: CaptureCallout;
    }> = [];

    try {
      for (const beat of beats) {
        const clipDir = resolve(clipsDir, `_raw-${beat.id}`);
        await tauriPage.startRecording({ path: clipDir, fps: 15 });

        await scriptedOpenTreePage(tauriPage, beat.label, {
          callout: beat.callout,
          hoverMs: 560,
          settleMs: beat.settleMs,
        });

        const stillPath = resolve(stillsDir, `${beat.id}.png`);
        const png = await tauriPage.screenshot({ path: stillPath });
        expect(
          png.length,
          `screenshot for ${beat.id} should be non-empty`,
        ).toBeGreaterThan(1_000);

        await sleep(beat.holdMs);
        await hideCaptureCallouts(tauriPage);
        await hideCaptureCursor(tauriPage);

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
          callout: beat.callout,
        });
      }
    } finally {
      await clearCaptureDirector(tauriPage).catch(() => undefined);
    }

    const manifest = {
      capturedAt: new Date().toISOString(),
      mode: "tauri-native",
      outRoot,
      overlay: process.env.LATTICE_DEV_DEMO_OVERLAY ?? null,
      director: {
        cursor: true,
        callouts: true,
      },
      beats: manifestBeats,
      note: "Native CoreGraphics capture via tauri-plugin-playwright. Private overlay + capture director via exec-for-dev --capture-yc.",
    };
    await writeFile(
      resolve(outRoot, "manifest.json"),
      `${JSON.stringify(manifest, null, 2)}\n`,
    );
  });
});
