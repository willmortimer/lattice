import { describe, expect, it, vi } from "vitest";

import type { JSONContent } from "@tiptap/core";

import { parseMarkdownToJSON, serializeJSONToMarkdown } from "./markdown";
import {
  applyModeSwitch,
  bodyForPersistence,
  draftBodyFromEdit,
  type ParseBody,
} from "./pageDraft";

const SAMPLE_JSON = parseMarkdownToJSON("# Hello\n\nParagraph.\n");
const SAMPLE_BODY = serializeJSONToMarkdown(SAMPLE_JSON);

describe("pageDraft mode switching", () => {
  it("serializes edit content into the draft when leaving edit mode", () => {
    const result = applyModeSwitch({
      from: "edit",
      to: "source",
      draftBody: "stale",
      editJson: SAMPLE_JSON,
    });

    expect(result.blocked).toBe(false);
    expect(result.mode).toBe("source");
    expect(result.draftBody).toBe(SAMPLE_BODY);
    expect(result.draftBody).not.toBe("stale");
  });

  it("round-trips edit → source → edit without changing the draft body", () => {
    const toSource = applyModeSwitch({
      from: "edit",
      to: "source",
      draftBody: "",
      editJson: SAMPLE_JSON,
    });
    const backToEdit = applyModeSwitch({
      from: "source",
      to: "edit",
      draftBody: toSource.draftBody,
      editJson: null,
    });

    expect(backToEdit.blocked).toBe(false);
    expect(backToEdit.mode).toBe("edit");
    expect(backToEdit.draftBody).toBe(SAMPLE_BODY);
    expect(backToEdit.editContent).toEqual(SAMPLE_JSON);
  });

  it("blocks source → edit when parsing fails and keeps the raw draft", () => {
    const raw = ":::broken-directive\n";
    const failingParse: ParseBody = () => ({ ok: false, error: "invalid node" });

    const result = applyModeSwitch(
      {
        from: "source",
        to: "edit",
        draftBody: raw,
        editJson: null,
      },
      failingParse,
    );

    expect(result.blocked).toBe(true);
    expect(result.mode).toBe("source");
    expect(result.draftBody).toBe(raw);
    expect(result.sourceParseError).toBe("invalid node");
    expect(result.editContent).toBeNull();
  });

  it("does not overwrite the draft when parse failure blocks edit", () => {
    const parse = vi
      .fn<ParseBody>()
      .mockReturnValueOnce({ ok: false, error: "blocked" })
      .mockReturnValueOnce({ ok: true, json: SAMPLE_JSON });

    const blocked = applyModeSwitch(
      { from: "source", to: "edit", draftBody: "keep me", editJson: null },
      parse,
    );
    const retry = applyModeSwitch(
      { from: "source", to: "edit", draftBody: blocked.draftBody, editJson: null },
      parse,
    );

    expect(blocked.draftBody).toBe("keep me");
    expect(retry.blocked).toBe(false);
    expect(retry.mode).toBe("edit");
    expect(parse).toHaveBeenCalledTimes(2);
  });

  it("uses live edit serialization for persistence while in edit mode", () => {
    const edited: JSONContent = parseMarkdownToJSON("## Updated\n");
    expect(bodyForPersistence("edit", "ignored", edited)).toBe(draftBodyFromEdit(edited));
    expect(bodyForPersistence("source", SAMPLE_BODY, edited)).toBe(SAMPLE_BODY);
    expect(bodyForPersistence("preview", SAMPLE_BODY, edited)).toBe(SAMPLE_BODY);
  });
});
