import { getSchema } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import { EditorState } from "@tiptap/pm/state";
import { history, undo } from "@tiptap/pm/history";
import { describe, expect, it } from "vitest";

import {
  createDictationProvisionalPlugin,
  DictationProvisional,
  dictationProvisionalKey,
} from "./DictationProvisional";
import { commitDictationFinalTransaction, parseDictationSegments } from "./dictationFinal";

const schema = getSchema([StarterKit, DictationProvisional]);

function createTestState(text = "Hello ") {
  return EditorState.create({
    schema,
    doc: schema.node("doc", null, [schema.node("paragraph", null, [schema.text(text)])]),
    plugins: [history(), createDictationProvisionalPlugin()],
  });
}

function docText(state: EditorState): string {
  return state.doc.textBetween(0, state.doc.content.size, "\n");
}

describe("parseDictationSegments", () => {
  it("returns plain text when no voice markers are present", () => {
    expect(parseDictationSegments("  hello world  ")).toEqual([
      { kind: "text", value: "hello world" },
    ]);
  });

  it("splits basic new line and new paragraph markers", () => {
    expect(parseDictationSegments("alpha new line beta new paragraph gamma")).toEqual([
      { kind: "text", value: "alpha" },
      { kind: "newline" },
      { kind: "text", value: "beta" },
      { kind: "paragraph" },
      { kind: "text", value: "gamma" },
    ]);
  });
});

describe("commitDictationFinalTransaction", () => {
  it("clears provisional decoration and inserts final text in one transaction", () => {
    const anchor = 7;
    let state = createTestState();

    state = state.apply(
      state.tr.setMeta(dictationProvisionalKey, { text: "world", from: anchor }),
    );
    expect(dictationProvisionalKey.getState(state)?.text).toBe("world");

    state = state.apply(commitDictationFinalTransaction(state, "world", anchor));

    expect(dictationProvisionalKey.getState(state)?.text).toBe("");
    expect(docText(state)).toBe("Hello world");
  });

  it("undoes the full final utterance in one step", () => {
    const anchor = 7;
    let state = createTestState();

    state = state.apply(commitDictationFinalTransaction(state, "world", anchor));
    expect(docText(state)).toBe("Hello world");

    let undone = state;
    const didUndo = undo(state, (tr) => {
      undone = state.apply(tr);
    });
    expect(didUndo).toBe(true);
    state = undone;

    expect(docText(state)).toBe("Hello ");
    expect(dictationProvisionalKey.getState(state)?.text).toBe("");
  });

  it("drops provisional state without mutating the document when final text is empty", () => {
    const anchor = 7;
    let state = createTestState();

    state = state.apply(
      state.tr.setMeta(dictationProvisionalKey, { text: "ghost", from: anchor }),
    );
    state = state.apply(commitDictationFinalTransaction(state, "   ", anchor));

    expect(dictationProvisionalKey.getState(state)?.text).toBe("");
    expect(docText(state)).toBe("Hello ");
  });

  it("clears provisional decoration when the document changes externally", () => {
    const anchor = 7;
    let state = createTestState();

    state = state.apply(
      state.tr.setMeta(dictationProvisionalKey, { text: "ghost", from: anchor }),
    );
    state = state.apply(state.tr.insertText("!", anchor));

    expect(dictationProvisionalKey.getState(state)?.text).toBe("");
  });
});
