/**
 * Shared YC scene script loader for native product capture.
 * Canonical file: lattice-ecosystem/demos/hackathon-pitch/scene.json
 */
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { CaptureCallout } from "./demoDirector";

const __dirname = dirname(fileURLToPath(import.meta.url));

export type ScenePresent = {
  enterFullscreen?: boolean;
  advanceSlides?: number;
  slideHoldMs?: number;
};

export type SceneBeat = {
  id: string;
  treeLabel: string;
  chapter?: string;
  title?: string;
  caption?: string;
  still?: string;
  clip?: string;
  requiresOverlay?: boolean;
  capture?: {
    settleMs?: number;
    holdMs?: number;
    callout?: CaptureCallout;
    present?: ScenePresent;
  };
};

export type SceneScript = {
  format?: string;
  version?: number;
  id?: string;
  title?: string;
  stage?: { width?: number; height?: number; fps?: number };
  voiceover?: { file?: string; volume?: number; optional?: boolean };
  remotion?: { beatFrames?: number; beatOverlap?: number };
  beats: SceneBeat[];
};

export function defaultScenePath(): string {
  return resolve(
    __dirname,
    "../../../../../demos/hackathon-pitch/scene.json",
  );
}

export async function loadSceneScript(
  path = process.env.LATTICE_DEMO_SCENE?.trim() || defaultScenePath(),
): Promise<SceneScript> {
  const raw = await readFile(path, "utf8");
  const parsed = JSON.parse(raw) as SceneScript;
  if (!Array.isArray(parsed.beats) || parsed.beats.length === 0) {
    throw new Error(`scene script missing beats: ${path}`);
  }
  return parsed;
}

export function beatsForCapture(
  scene: SceneScript,
  overlayEnabled: boolean,
): SceneBeat[] {
  return scene.beats.filter((beat) => !beat.requiresOverlay || overlayEnabled);
}
