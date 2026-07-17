import type { JSONContent } from "@tiptap/core";

import { serializeJSONToMarkdown, tryParseMarkdownToJSON, type MarkdownParseResult } from "./markdown";

export type PageMode = "edit" | "preview" | "source";

export interface PageDraftState {
  mode: PageMode;
  draftBody: string;
  sourceParseError: string | null;
}

export interface ModeSwitchInput {
  from: PageMode;
  to: PageMode;
  draftBody: string;
  editJson: JSONContent | null;
}

export interface ModeSwitchResult extends PageDraftState {
  blocked: boolean;
  editContent: JSONContent | null;
}

export type ParseBody = (body: string) => MarkdownParseResult;

/** Serialize the live editor document into the canonical draft body string. */
export function draftBodyFromEdit(editJson: JSONContent): string {
  return serializeJSONToMarkdown(editJson);
}

/**
 * Apply a page mode transition while keeping one canonical draft string.
 * Source→edit is blocked when markdown cannot be parsed into the page schema.
 */
export function applyModeSwitch(
  input: ModeSwitchInput,
  parseBody: ParseBody = tryParseMarkdownToJSON,
): ModeSwitchResult {
  const { from, to } = input;
  if (from === to) {
    return {
      mode: from,
      draftBody: input.draftBody,
      sourceParseError: null,
      blocked: false,
      editContent: null,
    };
  }

  let draftBody = input.draftBody;
  let sourceParseError: string | null = null;
  let editContent: JSONContent | null = null;

  if (from === "edit" && input.editJson) {
    draftBody = draftBodyFromEdit(input.editJson);
  }

  if (to === "edit") {
    const parsed = parseBody(draftBody);
    if (!parsed.ok) {
      return {
        mode: "source",
        draftBody,
        sourceParseError: parsed.error,
        blocked: true,
        editContent: null,
      };
    }
    editContent = parsed.json;
  }

  return {
    mode: to,
    draftBody,
    sourceParseError,
    blocked: false,
    editContent,
  };
}

/** Body text used for save/getRaw — always reflects the active mode's view of the draft. */
export function bodyForPersistence(
  mode: PageMode,
  draftBody: string,
  editJson: JSONContent | null,
): string {
  if (mode === "edit" && editJson) {
    return draftBodyFromEdit(editJson);
  }
  return draftBody;
}
