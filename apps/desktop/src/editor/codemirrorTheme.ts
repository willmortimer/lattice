import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import type { Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { tags, type Tag } from "@lezer/highlight";

function varToken(name: string): string {
  return `var(${name})`;
}

/** Tag styles mapped to Lattice semantic `--lt-*` tokens for unit tests. */
export const LATTICE_CODE_MIRROR_HIGHLIGHT_SPECS = [
  { tag: tags.meta, color: varToken("--lt-muted") },
  { tag: tags.link, color: varToken("--lt-accent"), textDecoration: "underline" },
  { tag: tags.heading, color: varToken("--lt-accent"), fontWeight: "bold" },
  { tag: tags.emphasis, fontStyle: "italic", color: varToken("--lt-text-soft") },
  { tag: tags.strong, fontWeight: "bold", color: varToken("--lt-text") },
  { tag: tags.strikethrough, textDecoration: "line-through", color: varToken("--lt-faint") },
  { tag: tags.keyword, color: varToken("--lt-accent") },
  {
    tag: [tags.atom, tags.bool, tags.url, tags.contentSeparator, tags.labelName],
    color: varToken("--lt-slate"),
  },
  { tag: [tags.literal, tags.inserted], color: varToken("--lt-accent-bright") },
  { tag: [tags.string, tags.deleted], color: varToken("--lt-accent-bright") },
  { tag: [tags.regexp, tags.escape, tags.special(tags.string)], color: varToken("--lt-accent") },
  { tag: tags.definition(tags.variableName), color: varToken("--lt-text") },
  { tag: tags.local(tags.variableName), color: varToken("--lt-text-soft") },
  { tag: [tags.typeName, tags.namespace], color: varToken("--lt-slate") },
  { tag: tags.className, color: varToken("--lt-slate") },
  { tag: [tags.special(tags.variableName), tags.macroName], color: varToken("--lt-accent-deep") },
  { tag: tags.definition(tags.propertyName), color: varToken("--lt-text-soft") },
  { tag: tags.propertyName, color: varToken("--lt-text-soft") },
  { tag: tags.comment, color: varToken("--lt-faint"), fontStyle: "italic" },
  { tag: tags.operator, color: varToken("--lt-muted") },
  { tag: tags.punctuation, color: varToken("--lt-faint") },
  { tag: tags.invalid, color: varToken("--lt-danger") },
] as const;

const latticeCodeMirrorHighlightStyle = HighlightStyle.define(LATTICE_CODE_MIRROR_HIGHLIGHT_SPECS);

const latticeCodeMirrorChromeTheme = EditorView.theme({
  "&.cm-focused .cm-cursor": {
    borderLeftColor: varToken("--lt-accent"),
  },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": {
    backgroundColor: varToken("--lt-accent-wash"),
  },
});

/** CodeMirror chrome + syntax highlighting extensions using Lattice theme tokens. */
export function latticeCodeMirrorTheme(): Extension[] {
  return [latticeCodeMirrorChromeTheme, syntaxHighlighting(latticeCodeMirrorHighlightStyle)];
}

export function highlightColorForTag(tag: Tag): string | undefined {
  for (const spec of LATTICE_CODE_MIRROR_HIGHLIGHT_SPECS) {
    const specTag = spec.tag;
    if (Array.isArray(specTag)) {
      if (specTag.includes(tag)) return spec.color;
    } else if (specTag === tag) {
      return spec.color;
    }
  }
  return undefined;
}
