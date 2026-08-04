import type { JSONContent } from "@tiptap/core";

import { joinFrontmatter } from "../markdown";
import { bodyForPersistence, type PageMode } from "../pageDraft";
import type { PageIO } from "../pageIO";
import { createSerializedSaveController, type SerializedSaveController } from "../serializedSave";
import type { PagePersistMode } from "./collabSession";

export interface MaterializePageInput {
  frontmatter: string | null;
  mode: PageMode;
  draftBody: string;
  editJson: JSONContent;
}

/** Build full page raw bytes for a collaborative checkpoint materialize save. */
export function buildMaterializedPageRaw(input: MaterializePageInput): string {
  const body = bodyForPersistence(input.mode, input.draftBody, input.editJson);
  return joinFrontmatter(input.frontmatter, body);
}

/** Whether editor edits should schedule a debounced markdown checkpoint (not per-keystroke save). */
export function shouldScheduleCollabCheckpoint(mode: PagePersistMode): boolean {
  return mode === "collaborative";
}

export interface CollabMaterializeDeps {
  getFrontmatter: () => string | null;
  getMode: () => PageMode;
  getDraftBody: () => string;
  getEditJson: () => JSONContent | null;
  io: PageIO;
}

/** Persist the current editor document as portable markdown for checkpoint/idle/close. */
export async function materializeCollabPage(
  deps: CollabMaterializeDeps,
  baseRevision: string | null,
): Promise<string | null> {
  const editJson = deps.getEditJson();
  if (!editJson) {
    throw new Error("No editor content to materialize");
  }
  const fullRaw = buildMaterializedPageRaw({
    frontmatter: deps.getFrontmatter(),
    mode: deps.getMode(),
    draftBody: deps.getDraftBody(),
    editJson,
  });
  return deps.io.save(fullRaw, baseRevision);
}

export type CollabMaterializeController = SerializedSaveController<string | null>;
