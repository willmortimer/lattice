/**
 * Shared YC scene script loader for native product capture.
 * Canonical file: lattice-ecosystem/demos/hackathon-pitch/scene.json
 */
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  beatsForCapture,
  parseSceneScript,
  type CaptureCallout,
  type SceneBeat,
  type ScenePresent,
  type SceneScript,
} from "./videoSchema";

const __dirname = dirname(fileURLToPath(import.meta.url));

export type { CaptureCallout, SceneBeat, ScenePresent, SceneScript };
export { beatsForCapture };

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
  return parseSceneScript(JSON.parse(raw));
}
