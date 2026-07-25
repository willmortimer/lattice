import { z } from "zod";

/** Maximum anchors per `overlay_show` event (Phase C MVP cap). */
export const MAX_OVERLAY_ANCHORS = 20;

export const markdownBlockAnchorSchema = z.object({
  kind: z.literal("markdown-block"),
  resourceId: z.string().min(1),
  revision: z.string().min(1).optional(),
  blockId: z.string().min(1),
});

export const datasetRegionAnchorSchema = z.object({
  kind: z.literal("dataset-region"),
  resourceId: z.string().min(1),
  revision: z.string().min(1).optional(),
  rowKeys: z.array(z.string().min(1)).min(1),
  columns: z.array(z.string().min(1)).optional(),
});

/**
 * Phase C MVP anchor union (`markdown-block` + `dataset-region` only).
 * Additional kinds from embedded-agent §11 land in later waves.
 */
export const workspaceAnchorSchema = z.discriminatedUnion("kind", [
  markdownBlockAnchorSchema,
  datasetRegionAnchorSchema,
]);

export type MarkdownBlockAnchor = z.infer<typeof markdownBlockAnchorSchema>;
export type DatasetRegionAnchor = z.infer<typeof datasetRegionAnchorSchema>;
export type WorkspaceAnchor = z.infer<typeof workspaceAnchorSchema>;

export function parseWorkspaceAnchor(value: unknown): WorkspaceAnchor {
  return workspaceAnchorSchema.parse(value);
}

export function serializeWorkspaceAnchor(anchor: WorkspaceAnchor): string {
  return JSON.stringify(anchor);
}
