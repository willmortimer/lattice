import { highlightingFor } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { tags } from "@lezer/highlight";
import { describe, expect, it } from "vitest";

import {
  highlightColorForTag,
  LATTICE_CODE_MIRROR_HIGHLIGHT_SPECS,
  latticeCodeMirrorTheme,
} from "./codemirrorTheme";

describe("latticeCodeMirrorTheme", () => {
  it("maps common syntax tags to var(--lt-*) colors", () => {
    expect(highlightColorForTag(tags.keyword)).toBe("var(--lt-accent)");
    expect(highlightColorForTag(tags.comment)).toBe("var(--lt-faint)");
    expect(highlightColorForTag(tags.string)).toBe("var(--lt-accent-bright)");
    expect(highlightColorForTag(tags.operator)).toBe("var(--lt-muted)");
    expect(highlightColorForTag(tags.typeName)).toBe("var(--lt-slate)");
    expect(highlightColorForTag(tags.meta)).toBe("var(--lt-muted)");
    expect(highlightColorForTag(tags.invalid)).toBe("var(--lt-danger)");
  });

  it("defines highlight specs using lattice CSS variables", () => {
    for (const spec of LATTICE_CODE_MIRROR_HIGHLIGHT_SPECS) {
      expect(spec.color).toMatch(/^var\(--lt-/);
    }
  });

  it("includes syntax highlighting in theme extensions", () => {
    const state = EditorState.create({
      doc: "const x = 1",
      extensions: latticeCodeMirrorTheme(),
    });
    expect(highlightingFor(state, [tags.keyword])).toBeTruthy();
    expect(highlightingFor(state, [tags.comment])).toBeTruthy();
  });
});
