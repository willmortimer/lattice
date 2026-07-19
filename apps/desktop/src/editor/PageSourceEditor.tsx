import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { useEffect, useRef } from "react";

import { latticeCodeMirrorTheme } from "./codemirrorTheme";

export interface PageSourceEditorProps {
  value: string;
  resetKey: string;
  onChange: (value: string) => void;
}

export function PageSourceEditor({ value, resetKey, onChange }: PageSourceEditorProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const controller = new AbortController();
    let view: EditorView | null = null;
    let disposed = false;

    void import("@codemirror/lang-markdown")
      .then(({ markdown: loadMarkdown }) => {
        if (disposed || controller.signal.aborted) return;
        view = new EditorView({
          state: EditorState.create({
            doc: value,
            extensions: [
              ...latticeCodeMirrorTheme(),
              history(),
              keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
              loadMarkdown(),
              EditorView.lineWrapping,
              EditorView.updateListener.of((update) => {
                if (update.docChanged) {
                  onChangeRef.current(update.state.doc.toString());
                }
              }),
            ],
          }),
          parent: host,
        });
      })
      .catch(() => {
        // A cancelled chunk leaves the host empty; remount via resetKey to retry.
      });

    return () => {
      disposed = true;
      controller.abort();
      view?.destroy();
      host.replaceChildren();
    };
  }, [resetKey, value]);

  return (
    <div
      className="page-source-editor"
      ref={hostRef}
      aria-label="Page source"
      aria-multiline="true"
    />
  );
}
