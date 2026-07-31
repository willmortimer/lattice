/**
 * Native product capture for Remotion (Tauri window via tauri-plugin-playwright).
 *
 * Driven by the shared ecosystem scene script
 * (`demos/hackathon-pitch/scene.json` via `LATTICE_DEMO_SCENE`).
 *
 * Preferred:
 *
 *   ./scripts/exec-for-dev.sh --capture-yc
 *
 * Records clean product footage + event tracks (cursor/callouts rendered in Remotion).
 */
import { access, mkdir, rename, writeFile } from "node:fs/promises";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { TauriPage } from "@srsholmes/tauri-playwright";
import { waitForShellChrome } from "../perf/helpers";
import { expect, test } from "../fixtures";
import {
  scriptedOpenTreePage,
  waitForOverlayTreeLabel,
  type CaptureEvent,
} from "./demoDirector";
import { beatsForCapture, loadSceneScript } from "./sceneScript";
import { montageDurationFrames, parseCaptureManifest } from "./videoSchema";

const __dirname = dirname(fileURLToPath(import.meta.url));

const DEFAULT_OUT = resolve(
  __dirname,
  "../../../../../apps/yc-remotion/public/product",
);

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

async function fileExists(path: string): Promise<boolean> {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
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
    const eventsDir = resolve(outRoot, "events");
    await mkdir(stillsDir, { recursive: true });
    await mkdir(clipsDir, { recursive: true });
    await mkdir(eventsDir, { recursive: true });

    const fps = scene.stage.fps;
    const beatFrames = scene.remotion.beatFrames;
    const transitionFrames =
      scene.remotion.transitionFrames ?? scene.remotion.beatOverlap;

    tauriPage.setDefaultTimeout(45_000);
    await waitForShellChrome(tauriPage);
    await applyStage(tauriPage);

    const overlayEnabled = Boolean(process.env.LATTICE_DEV_DEMO_OVERLAY?.trim());
    const planned = beatsForCapture(scene, overlayEnabled);
    if (overlayEnabled) {
      const pitch = scene.beats.find((beat) => beat.requiresOverlay);
      if (pitch) {
        const ready = await waitForOverlayTreeLabel(tauriPage, pitch.treeLabel);
        if (!ready) {
          throw new Error(
            `capture: overlay set but required tree label missing: ${pitch.treeLabel}`,
          );
        }
      }
    }

    const manifestBeats: Array<Record<string, unknown>> = [];
    const skippedRequired: string[] = [];

    for (const beat of planned) {
      const required = beat.required !== false;
      if (beat.requiresOverlay) {
        const ready = await waitForOverlayTreeLabel(tauriPage, beat.treeLabel);
        if (!ready) {
          if (required) skippedRequired.push(beat.id);
          continue;
        }
      }

      const clipDir = resolve(clipsDir, `_raw-${beat.id}`);
      const recordingStartedAtMs = Date.now();
      await tauriPage.startRecording({ path: clipDir, fps });

      const events: CaptureEvent[] = await scriptedOpenTreePage(
        tauriPage,
        beat.treeLabel,
        {
          callout: beat.capture?.callout,
          hoverMs: 560,
          settleMs: beat.capture?.settleMs ?? 900,
          recordingStartedAtMs,
        },
      );

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

      const recording = await tauriPage.stopRecording();
      if (!recording.video) {
        throw new Error(`capture: no video for required beat ${beat.id}`);
      }
      const ext = extname(recording.video).toLowerCase() || ".mp4";
      const clipName = `${beat.id}${ext === ".webm" ? ".webm" : ".mp4"}`;
      const clipRel = `product/clips/${clipName}`;
      await rename(recording.video, resolve(clipsDir, clipName));

      const eventsRel = `product/events/${beat.id}.json`;
      await writeFile(
        resolve(eventsDir, `${beat.id}.json`),
        `${JSON.stringify(events, null, 2)}\n`,
      );

      manifestBeats.push({
        id: beat.id,
        label: beat.treeLabel,
        chapter: beat.chapter ?? null,
        title: beat.title ?? null,
        caption: beat.caption ?? null,
        still: beat.still ?? `product/stills/${beat.id}.png`,
        clip: clipRel,
        events: eventsRel,
        callout: beat.capture?.callout ?? null,
        present: beat.capture?.present ?? null,
        required,
      });
    }

    if (skippedRequired.length > 0) {
      throw new Error(
        `capture: skipped required beats: ${skippedRequired.join(", ")}`,
      );
    }
    if (manifestBeats.length === 0) {
      throw new Error("capture: no beats captured");
    }

    for (const beat of manifestBeats) {
      const clipPath = resolve(
        outRoot,
        String(beat.clip).replace(/^product\//, ""),
      );
      const stillPath = resolve(
        outRoot,
        String(beat.still).replace(/^product\//, ""),
      );
      if (!(await fileExists(clipPath))) {
        throw new Error(`capture: missing clip on disk: ${clipPath}`);
      }
      if (!(await fileExists(stillPath))) {
        throw new Error(`capture: missing still on disk: ${stillPath}`);
      }
    }

    const chapters = manifestBeats.map((beat, index) => {
      const from = index * (beatFrames - transitionFrames);
      return {
        id: beat.id,
        title: (beat.chapter as string | null) ?? String(beat.id),
        startFrame: from,
        startSeconds: Number((from / fps).toFixed(3)),
      };
    });

    const manifest = parseCaptureManifest({
      format: "lattice-capture-manifest",
      version: 1,
      capturedAt: new Date().toISOString(),
      mode: "tauri-native",
      sceneId: scene.id ?? null,
      scenePath: process.env.LATTICE_DEMO_SCENE ?? null,
      outRoot,
      overlay: process.env.LATTICE_DEV_DEMO_OVERLAY ?? null,
      stage: scene.stage,
      remotion: {
        beatFrames,
        beatOverlap: scene.remotion.beatOverlap,
        transitionFrames,
      },
      director: {
        cursor: true,
        callouts: true,
        bakedIntoFootage: false,
        present: true,
        stage: true,
      },
      voiceover: scene.voiceover ?? null,
      chapters,
      beats: manifestBeats,
      note: "Authoritative capture manifest. Remotion must fail closed on missing clips.",
    });

    await writeFile(
      resolve(outRoot, "manifest.json"),
      `${JSON.stringify(manifest, null, 2)}\n`,
    );
    await writeFile(
      resolve(outRoot, "chapters.json"),
      `${JSON.stringify(
        {
          fps,
          durationInFrames: montageDurationFrames(
            manifestBeats.length,
            beatFrames,
            transitionFrames,
          ),
          chapters,
          voiceover: scene.voiceover ?? null,
          source: "capture-manifest",
        },
        null,
        2,
      )}\n`,
    );
  });
});
