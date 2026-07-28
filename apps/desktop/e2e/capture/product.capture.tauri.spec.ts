/**
 * Native product capture for Remotion (Tauri window via tauri-plugin-playwright).
 *
 * Driven by the shared ecosystem scene script
 * (`demos/hackathon-pitch/scene.json` via `LATTICE_DEMO_SCENE`).
 *
 * Preferred:
 *
 *   ./scripts/exec-for-dev.sh --capture-yc
 */
import { mkdir, rename, writeFile } from "node:fs/promises";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { TauriPage } from "@srsholmes/tauri-playwright";
import { waitForShellChrome } from "../perf/helpers";
import { expect, test } from "../fixtures";
import {
  clearCaptureDirector,
  hideCaptureCallouts,
  hideCaptureCursor,
  installCaptureDirector,
  scriptedOpenTreePage,
  waitForOverlayTreeLabel,
} from "./demoDirector";
import { beatsForCapture, loadSceneScript } from "./sceneScript";

const __dirname = dirname(fileURLToPath(import.meta.url));

const DEFAULT_OUT = resolve(
  __dirname,
  "../../../../../apps/yc-remotion/public/product",
);

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

async function applyStage(page: TauriPage): Promise<void> {
  await page.evaluate(`(async () => {
    const invoke = window.__TAURI_INTERNALS__?.invoke;
    if (typeof invoke === "function") {
      await invoke("apply_demo_stage");
    }
  })()`);
  await sleep(400);
}

async function presentDeck(
  page: TauriPage,
  present: {
    enterFullscreen?: boolean;
    advanceSlides?: number;
    slideHoldMs?: number;
  },
): Promise<void> {
  if (present.enterFullscreen) {
    await page.waitForSelector('[aria-label$=" presentation"]', 30_000);
    const clicked = await page.evaluate(`(() => {
      const button = Array.from(document.querySelectorAll("button"))
        .find((el) => el.textContent?.trim() === "Fullscreen");
      if (!button) return false;
      button.click();
      return true;
    })()`);
    expect(clicked, "Fullscreen control should exist for deck present").toBe(true);
    await sleep(600);
  }
  const slides = Math.max(0, present.advanceSlides ?? 0);
  const hold = present.slideHoldMs ?? 1600;
  for (let i = 0; i < slides; i++) {
    await page.evaluate(`window.dispatchEvent(new KeyboardEvent("keydown", {
      key: "ArrowRight",
      bubbles: true,
      cancelable: true,
    }))`);
    await sleep(hold);
  }
}

test.describe("YC product capture (tauri native)", () => {
  test.setTimeout(360_000);

  test("captures stills and short clips from shared scene script", async ({
    tauriPage,
  }) => {
    const scene = await loadSceneScript();
    const outRoot = resolve(process.env.YC_REMOTION_ASSETS ?? DEFAULT_OUT);
    const stillsDir = resolve(outRoot, "stills");
    const clipsDir = resolve(outRoot, "clips");
    await mkdir(stillsDir, { recursive: true });
    await mkdir(clipsDir, { recursive: true });

    tauriPage.setDefaultTimeout(45_000);
    await waitForShellChrome(tauriPage);
    await applyStage(tauriPage);
    await installCaptureDirector(tauriPage);

    const overlayEnabled = Boolean(process.env.LATTICE_DEV_DEMO_OVERLAY?.trim());
    const beats = beatsForCapture(scene, overlayEnabled);
    if (overlayEnabled) {
      const pitch = scene.beats.find((beat) => beat.requiresOverlay);
      if (pitch) {
        const ready = await waitForOverlayTreeLabel(tauriPage, pitch.treeLabel);
        if (!ready) {
          console.warn(
            `capture: overlay set but ${pitch.treeLabel} missing; skipping overlay beats`,
          );
        }
      }
    }

    const manifestBeats: Array<Record<string, unknown>> = [];

    try {
      for (const beat of beats) {
        if (beat.requiresOverlay) {
          const ready = await waitForOverlayTreeLabel(tauriPage, beat.treeLabel);
          if (!ready) continue;
        }

        const clipDir = resolve(clipsDir, `_raw-${beat.id}`);
        await tauriPage.startRecording({ path: clipDir, fps: 15 });

        await scriptedOpenTreePage(tauriPage, beat.treeLabel, {
          callout: beat.capture?.callout,
          hoverMs: 560,
          settleMs: beat.capture?.settleMs ?? 900,
        });

        if (beat.capture?.present) {
          await presentDeck(tauriPage, beat.capture.present);
        } else {
          await sleep(beat.capture?.holdMs ?? 2000);
        }

        const stillPath = resolve(stillsDir, `${beat.id}.png`);
        const png = await tauriPage.screenshot({ path: stillPath });
        expect(
          png.length,
          `screenshot for ${beat.id} should be non-empty`,
        ).toBeGreaterThan(1_000);

        await hideCaptureCallouts(tauriPage);
        await hideCaptureCursor(tauriPage);

        const recording = await tauriPage.stopRecording();
        // Playwright / tauri-plugin may emit .webm or .mp4; Remotion accepts both.
        let clipRel: string | null = null;
        if (recording.video) {
          const ext = extname(recording.video).toLowerCase() || ".mp4";
          const clipName = `${beat.id}${ext === ".webm" ? ".webm" : ".mp4"}`;
          clipRel = `product/clips/${clipName}`;
          await rename(recording.video, resolve(clipsDir, clipName));
        }

        manifestBeats.push({
          id: beat.id,
          label: beat.treeLabel,
          chapter: beat.chapter ?? null,
          still: beat.still ?? `product/stills/${beat.id}.png`,
          clip: clipRel,
          callout: beat.capture?.callout ?? null,
          present: beat.capture?.present ?? null,
        });
      }
    } finally {
      await clearCaptureDirector(tauriPage).catch(() => undefined);
    }

    const beatFrames = scene.remotion?.beatFrames ?? 90;
    const beatOverlap = scene.remotion?.beatOverlap ?? 12;
    const chapters = manifestBeats.map((beat, index) => {
      const from = index * (beatFrames - beatOverlap);
      return {
        id: beat.id,
        title: beat.chapter ?? beat.id,
        startFrame: from,
        startSeconds: Number((from / (scene.stage?.fps ?? 30)).toFixed(3)),
      };
    });

    const manifest = {
      capturedAt: new Date().toISOString(),
      mode: "tauri-native",
      sceneId: scene.id ?? null,
      scenePath: process.env.LATTICE_DEMO_SCENE ?? null,
      outRoot,
      overlay: process.env.LATTICE_DEV_DEMO_OVERLAY ?? null,
      director: { cursor: true, callouts: true, present: true, stage: true },
      voiceover: scene.voiceover ?? null,
      chapters,
      beats: manifestBeats,
      note: "Shared scene script + capture director via exec-for-dev --capture-yc.",
    };
    await writeFile(
      resolve(outRoot, "manifest.json"),
      `${JSON.stringify(manifest, null, 2)}\n`,
    );
    await writeFile(
      resolve(outRoot, "chapters.json"),
      `${JSON.stringify({ fps: scene.stage?.fps ?? 30, chapters }, null, 2)}\n`,
    );
  });
});
