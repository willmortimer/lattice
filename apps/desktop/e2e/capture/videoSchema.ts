/**
 * Zod schemas for product capture (mirrors ecosystem `@lattice/video-schema`).
 * Keep in sync with `packages/video-schema` in lattice-ecosystem.
 */
import { z } from "zod";

export const CaptureCalloutSchema = z.object({
  id: z.string().min(1).optional(),
  title: z.string().min(1).max(120),
  body: z.string().max(280).optional(),
  anchorSelector: z.string().min(1).optional(),
  placement: z.enum(["top", "bottom", "left", "right"]).optional(),
});

export const ScenePresentSchema = z.object({
  enterFullscreen: z.boolean().optional(),
  advanceSlides: z.number().int().nonnegative().optional(),
  slideHoldMs: z.number().int().positive().optional(),
});

export const SceneCaptureSchema = z.object({
  settleMs: z.number().int().positive().optional(),
  holdMs: z.number().int().positive().optional(),
  callout: CaptureCalloutSchema.optional(),
  present: ScenePresentSchema.optional(),
});

export const SceneBeatSchema = z.object({
  id: z.string().min(1),
  treeLabel: z.string().min(1),
  chapter: z.string().min(1).optional(),
  title: z.string().min(1).max(120).optional(),
  caption: z.string().max(280).optional(),
  still: z.string().min(1).optional(),
  clip: z.string().min(1).optional(),
  requiresOverlay: z.boolean().optional(),
  required: z.boolean().optional(),
  capture: SceneCaptureSchema.optional(),
});

export const SceneScriptSchema = z
  .object({
    format: z.literal("lattice-yc-scene"),
    version: z.number().int().positive(),
    id: z.string().min(1),
    title: z.string().min(1),
    stage: z.object({
      width: z.number().int().positive(),
      height: z.number().int().positive(),
      fps: z.number().int().positive(),
    }),
    voiceover: z
      .object({
        file: z.string().min(1),
        volume: z.number().min(0).max(1).optional(),
        optional: z.boolean().optional(),
        captions: z.string().min(1).optional(),
        required: z.boolean().optional(),
      })
      .optional(),
    remotion: z.object({
      beatFrames: z.number().int().positive(),
      beatOverlap: z.number().int().nonnegative(),
      transitionFrames: z.number().int().positive().optional(),
    }),
    beats: z.array(SceneBeatSchema).min(1),
  })
  .superRefine((scene, ctx) => {
    const ids = new Set<string>();
    for (const [index, beat] of scene.beats.entries()) {
      if (ids.has(beat.id)) {
        ctx.addIssue({
          code: "custom",
          message: `duplicate beat id "${beat.id}"`,
          path: ["beats", index, "id"],
        });
      }
      ids.add(beat.id);
    }
  });

export const CaptureManifestSchema = z.object({
  format: z.literal("lattice-capture-manifest"),
  version: z.literal(1),
  capturedAt: z.string().min(1),
  mode: z.enum(["tauri-native", "browser-demo"]),
  sceneId: z.string().nullable().optional(),
  scenePath: z.string().nullable().optional(),
  outRoot: z.string().min(1).optional(),
  overlay: z.string().nullable().optional(),
  stage: z
    .object({
      width: z.number().int().positive(),
      height: z.number().int().positive(),
      fps: z.number().int().positive(),
    })
    .optional(),
  remotion: z
    .object({
      beatFrames: z.number().int().positive(),
      beatOverlap: z.number().int().nonnegative(),
      transitionFrames: z.number().int().positive().optional(),
    })
    .optional(),
  voiceover: z
    .object({
      file: z.string().min(1),
      volume: z.number().min(0).max(1).optional(),
      optional: z.boolean().optional(),
    })
    .nullable()
    .optional(),
  director: z
    .object({
      cursor: z.boolean(),
      callouts: z.boolean(),
      bakedIntoFootage: z.boolean(),
      present: z.boolean().optional(),
      stage: z.boolean().optional(),
    })
    .optional(),
  chapters: z.array(
    z.object({
      id: z.string().min(1),
      title: z.string().min(1),
      startFrame: z.number().int().nonnegative(),
      startSeconds: z.number().nonnegative(),
    }),
  ),
  beats: z
    .array(
      z.object({
        id: z.string().min(1),
        label: z.string().min(1),
        chapter: z.string().nullable().optional(),
        title: z.string().nullable().optional(),
        caption: z.string().nullable().optional(),
        still: z.string().min(1),
        clip: z.string().min(1),
        events: z.string().min(1).optional(),
        callout: CaptureCalloutSchema.nullable().optional(),
        present: ScenePresentSchema.nullable().optional(),
        required: z.boolean().optional(),
      }),
    )
    .min(1),
  note: z.string().optional(),
});

export type CaptureCallout = z.infer<typeof CaptureCalloutSchema>;
export type ScenePresent = z.infer<typeof ScenePresentSchema>;
export type SceneBeat = z.infer<typeof SceneBeatSchema>;
export type SceneScript = z.infer<typeof SceneScriptSchema>;
export type CaptureManifest = z.infer<typeof CaptureManifestSchema>;

export function parseSceneScript(input: unknown): SceneScript {
  return SceneScriptSchema.parse(input);
}

export function parseCaptureManifest(input: unknown): CaptureManifest {
  return CaptureManifestSchema.parse(input);
}

export function beatsForCapture(
  scene: SceneScript,
  overlayEnabled: boolean,
): SceneBeat[] {
  return scene.beats.filter((beat) => !beat.requiresOverlay || overlayEnabled);
}

export function montageDurationFrames(
  beatCount: number,
  beatFrames: number,
  transitionFrames: number,
): number {
  if (beatCount <= 0) return 0;
  return beatCount * beatFrames - (beatCount - 1) * transitionFrames;
}
