import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import type { EditorState, Transaction } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

export type DictationProvisionalState = {
  text: string;
  from: number;
};

export const dictationProvisionalKey = new PluginKey<DictationProvisionalState>(
  "latticeDictationProvisional",
);

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    dictationProvisional: {
      setDictationProvisional: (text: string, from: number) => ReturnType;
      clearDictationProvisional: () => ReturnType;
    };
  }
}

/**
 * Ghost provisional transcript. Decoration only — never enters the document,
 * undo stack, or autosave path (voice ADR 0005 / M2).
 */
export const DictationProvisional = Extension.create({
  name: "dictationProvisional",

  addCommands() {
    return {
      setDictationProvisional:
        (text: string, from: number) =>
        ({ tr, dispatch }) => {
          if (dispatch) {
            tr.setMeta(dictationProvisionalKey, { text, from } satisfies DictationProvisionalState);
            dispatch(tr);
          }
          return true;
        },
      clearDictationProvisional:
        () =>
        ({ tr, dispatch }) => {
          if (dispatch) {
            tr.setMeta(dictationProvisionalKey, {
              text: "",
              from: 0,
            } satisfies DictationProvisionalState);
            dispatch(tr);
          }
          return true;
        },
    };
  },

  addProseMirrorPlugins() {
    return [
      new Plugin<DictationProvisionalState>({
        key: dictationProvisionalKey,
        state: {
          init: () => ({ text: "", from: 0 }),
          apply(tr: Transaction, value: DictationProvisionalState) {
            const meta = tr.getMeta(dictationProvisionalKey) as
              | DictationProvisionalState
              | undefined;
            if (meta) return meta;
            if (!value.text) return value;
            // Drop provisional across doc changes that aren't our own meta updates
            // (e.g. the final insert transaction if meta were missing).
            if (tr.docChanged) return { text: "", from: 0 };
            return { text: value.text, from: tr.mapping.map(value.from) };
          },
        },
        props: {
          decorations(state: EditorState) {
            const value = dictationProvisionalKey.getState(state);
            if (!value?.text) return DecorationSet.empty;
            const from = Math.min(Math.max(1, value.from), state.doc.content.size);
            // Key forces widget DOM recreation when text changes so stale italics
            // cannot linger beside the inserted final.
            return DecorationSet.create(state.doc, [
              Decoration.widget(
                from,
                () => {
                  const span = document.createElement("span");
                  span.className = "dictation-provisional";
                  span.setAttribute("aria-live", "polite");
                  span.setAttribute("data-dictation-provisional", "true");
                  span.textContent = value.text;
                  return span;
                },
                { side: 1, key: `dictation-provisional:${value.from}:${value.text}` },
              ),
            ]);
          },
        },
      }),
    ];
  },
});
