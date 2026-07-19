import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import type { LanguageSupport } from "@codemirror/language";
import { useEffect, useRef } from "react";

import { latticeCodeMirrorTheme } from "../../editor/codemirrorTheme";

export type TextSyntax = "plain-text" | "code" | "json" | "yaml";

export interface TextCodeMirrorProps {
  initialValue: string;
  syntax: TextSyntax;
  language?: string;
  readOnly: boolean;
  resetKey: string;
  onChange: (value: string) => void;
}

function abortableImport<T>(load: () => Promise<T>, signal: AbortSignal): Promise<T> {
  if (signal.aborted) return Promise.reject(new DOMException("Language load cancelled", "AbortError"));
  return load().then((value) => {
    if (signal.aborted) throw new DOMException("Language load cancelled", "AbortError");
    return value;
  });
}

async function loadLanguage(syntax: TextSyntax, language: string | undefined, signal: AbortSignal): Promise<LanguageSupport | null> {
  if (syntax === "plain-text") return null;
  if (syntax === "json") return abortableImport(() => import("@codemirror/lang-json").then(({ json }) => json()), signal);
  if (syntax === "yaml") return abortableImport(() => import("@codemirror/lang-yaml").then(({ yaml }) => yaml()), signal);

  switch (language) {
    case "javascript":
    case "typescript":
      return abortableImport(() => import("@codemirror/lang-javascript").then(({ javascript }) => javascript({ jsx: language === "javascript" })), signal);
    case "python":
      return abortableImport(() => import("@codemirror/lang-python").then(({ python }) => python()), signal);
    case "rust":
      return abortableImport(() => import("@codemirror/lang-rust").then(({ rust }) => rust()), signal);
    case "sql":
      return abortableImport(() => import("@codemirror/lang-sql").then(({ sql }) => sql()), signal);
    case "css":
      return abortableImport(() => import("@codemirror/lang-css").then(({ css }) => css()), signal);
    case "html":
      return abortableImport(() => import("@codemirror/lang-html").then(({ html }) => html()), signal);
    default:
      return null;
  }
}

export function TextCodeMirror({ initialValue, syntax, language, readOnly, resetKey, onChange }: TextCodeMirrorProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const controller = new AbortController();
    let view: EditorView | null = null;
    let disposed = false;

    void loadLanguage(syntax, language, controller.signal).then((languageSupport) => {
      if (disposed || controller.signal.aborted) return;
      const extensions = [
        ...latticeCodeMirrorTheme(),
        history(),
        keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
        EditorState.readOnly.of(readOnly),
        EditorView.editable.of(!readOnly),
        EditorView.lineWrapping,
        EditorView.updateListener.of((update) => {
          if (update.docChanged && update.view.state.facet(EditorState.readOnly) === false) {
            onChangeRef.current(update.state.doc.toString());
          }
        }),
      ];
      if (languageSupport) extensions.push(languageSupport);
      view = new EditorView({
        state: EditorState.create({ doc: initialValue, extensions }),
        parent: host,
      });
    }).catch(() => {
      // A cancelled/lost language chunk leaves the host empty; the parent
      // remains able to show the source or retry after a remount.
    });

    return () => {
      disposed = true;
      controller.abort();
      view?.destroy();
      host.replaceChildren();
    };
  }, [initialValue, language, readOnly, resetKey, syntax]);

  return <div className="lattice-text-editor" ref={hostRef} aria-label={`${syntax} editor`} aria-multiline="true" />;
}

export function syntaxForPath(path: string, formatId?: string): { syntax: TextSyntax; language?: string } {
  const extension = path.split(".").pop()?.toLowerCase() ?? "";
  if (formatId === "json" || extension === "json") return { syntax: "json" };
  if (formatId === "yaml" || ["yaml", "yml"].includes(extension)) return { syntax: "yaml" };
  const languages: Record<string, string> = {
    js: "javascript", jsx: "javascript", mjs: "javascript", cjs: "javascript",
    ts: "typescript", tsx: "typescript", py: "python", rs: "rust", sql: "sql",
    css: "css", html: "html", htm: "html",
  };
  return languages[extension] ? { syntax: "code", language: languages[extension] } : { syntax: formatId === "code" ? "code" : "plain-text" };
}
