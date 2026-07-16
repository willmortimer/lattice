/**
 * Lattice's markdown codec.
 *
 * Rather than delegate to a third-party markdown extension, Lattice owns
 * both directions of the parse/serialize boundary so the on-disk dialect
 * stays exactly what docs/07 and ADR 0003 commit to: conservative
 * CommonMark/GFM (headings, lists, blockquotes, fenced code, tables,
 * links, emphasis/strong/strike/inline-code) with YAML front matter kept
 * verbatim and untouched.
 *
 * `markdown-it` (via `prosemirror-markdown`'s generic `MarkdownParser`)
 * tokenizes; `MarkdownSerializer` walks the ProseMirror doc back out. Both
 * are configured against the schema built from `editorExtensions`
 * (`extensions.ts`), so the dialect and the editor's node/mark set can
 * never drift apart.
 *
 * The codec speaks `JSONContent` (Tiptap's plain-data doc shape) at its
 * public boundary, not live ProseMirror `Node`s, so callers can hand the
 * result straight to `useEditor({ content })` / `editor.getJSON()`
 * without worrying about schema-instance identity between this module's
 * standalone schema and the one a live `Editor` builds for itself.
 */
import type { JSONContent } from "@tiptap/core";
import { getSchema } from "@tiptap/core";
import MarkdownIt from "markdown-it";
import type Token from "markdown-it/lib/token.mjs";
import type { Node as PMNode } from "prosemirror-model";
import {
  MarkdownParser,
  MarkdownSerializer,
  MarkdownSerializerState,
  type ParseSpec,
} from "prosemirror-markdown";

import { editorExtensions } from "./extensions";

const schema = getSchema(editorExtensions);

// ---------------------------------------------------------------------------
// Parsing: markdown -> JSONContent
// ---------------------------------------------------------------------------

/** Reads a GFM alignment column's `style="text-align:…"` attribute. */
function tableAlign(token: Token): "left" | "center" | "right" | null {
  const style = token.attrGet("style");
  const match = style ? /text-align:(left|center|right)/.exec(style) : null;
  return (match?.[1] as "left" | "center" | "right" | undefined) ?? null;
}

const tokenizer = new MarkdownIt("commonmark", { html: false }).enable([
  "table",
  "strikethrough",
]);

/**
 * markdown-it emits table cells as a bare `inline` token (no block wrapper),
 * but our schema's `tableCell`/`tableHeader` require block content (`block+`,
 * matching Tiptap's table extension). Wrap each cell's inline token in a
 * synthetic paragraph so it satisfies the schema — this runs as a core rule
 * so it sees the real token stream before `MarkdownParser` walks it.
 */
tokenizer.core.ruler.push("lattice-wrap-table-cells", (state) => {
  const input = state.tokens;
  const output: typeof input = [];
  for (let i = 0; i < input.length; i++) {
    const token = input[i];
    output.push(token);
    if (token.type !== "th_open" && token.type !== "td_open") continue;
    const inline = input[i + 1];
    if (!inline || inline.type !== "inline") continue;
    const paragraphOpen = new state.Token("paragraph_open", "p", 1);
    const paragraphClose = new state.Token("paragraph_close", "p", -1);
    output.push(paragraphOpen, inline, paragraphClose);
    i += 1;
  }
  state.tokens = output;
});

const parserTokens: { [tokenType: string]: ParseSpec } = {
  paragraph: { block: "paragraph" },
  blockquote: { block: "blockquote" },
  list_item: { block: "listItem" },
  bullet_list: { block: "bulletList" },
  ordered_list: {
    block: "orderedList",
    getAttrs: (tok) => ({ start: Number(tok.attrGet("start")) || 1 }),
  },
  heading: { block: "heading", getAttrs: (tok) => ({ level: Number(tok.tag.slice(1)) }) },
  code_block: { block: "codeBlock", noCloseToken: true },
  fence: {
    block: "codeBlock",
    getAttrs: (tok) => ({ language: tok.info || null }),
    noCloseToken: true,
  },
  hr: { node: "horizontalRule" },
  hardbreak: { node: "hardBreak" },
  image: {
    node: "image",
    getAttrs: (tok) => ({
      src: tok.attrGet("src"),
      alt: tok.attrGet("alt") || tok.content || null,
      title: tok.attrGet("title") || null,
    }),
  },
  em: { mark: "italic" },
  strong: { mark: "bold" },
  s: { mark: "strike" },
  link: {
    mark: "link",
    getAttrs: (tok) => ({
      href: tok.attrGet("href"),
      title: tok.attrGet("title") || null,
    }),
  },
  code_inline: { mark: "code", noCloseToken: true },
  // GFM tables. `thead`/`tbody` only group rows in markdown-it's token
  // stream; our schema has no equivalent node, so they're ignored wrappers
  // (the `tr`/`th`/`td` tokens inside still parse as direct table children).
  table: { block: "table" },
  thead: { ignore: true },
  tbody: { ignore: true },
  tr: { block: "tableRow" },
  th: { block: "tableHeader", getAttrs: tableAlign as ParseSpec["getAttrs"] },
  td: { block: "tableCell", getAttrs: tableAlign as ParseSpec["getAttrs"] },
};

const parser = new MarkdownParser(schema, tokenizer, parserTokens);

/** Turn `[[Target]]` / `[[Target|label]]` into markdown links with a `wiki:` href. */
function encodeWikiLinks(markdown: string): string {
  return markdown.replace(/\[\[([^\]|\n]+)(?:\|([^\]\n]+))?\]\]/g, (_full, target, label) => {
    const t = String(target).trim();
    const text = (label != null ? String(label) : t).trim();
    return `[${text}](wiki:${encodeURIComponent(t)})`;
  });
}

/** Parse a page body (frontmatter already stripped) into Tiptap JSON. */
export function parseMarkdownToJSON(markdown: string): JSONContent {
  return parser.parse(encodeWikiLinks(markdown)).toJSON() as JSONContent;
}

// ---------------------------------------------------------------------------
// Serializing: JSONContent -> markdown
// ---------------------------------------------------------------------------

/** The longest run of backticks in `node`'s text, used to fence inline code. */
function longestBacktickRun(node: PMNode): number {
  if (!node.isText || !node.text) return 0;
  const runs = node.text.match(/`+/g);
  return runs ? Math.max(...runs.map((run) => run.length)) : 0;
}

/** Inline-code delimiter that is guaranteed not to appear in `node`'s text. */
function codeFence(node: PMNode, side: "open" | "close"): string {
  const len = longestBacktickRun(node);
  if (len === 0) return "`";
  const ticks = "`".repeat(len + 1);
  return side === "open" ? ticks + " " : " " + ticks;
}

const serializerMarks: MarkdownSerializer["marks"] = {
  bold: { open: "**", close: "**", mixable: true, expelEnclosingWhitespace: true },
  italic: { open: "*", close: "*", mixable: true, expelEnclosingWhitespace: true },
  strike: { open: "~~", close: "~~", mixable: true, expelEnclosingWhitespace: true },
  code: {
    open: (_state, _mark, parent, index) => codeFence(parent.child(index), "open"),
    close: (_state, _mark, parent, index) => codeFence(parent.child(index - 1), "close"),
    escape: false,
  },
  link: {
    open: (_state, mark) => {
      const href = mark.attrs.href as string;
      if (href.startsWith("wiki:")) {
        const target = decodeURIComponent(href.slice("wiki:".length));
        // `[[target|` + label + `]]`; collapsed to `[[target]]` when label matches.
        return `[[${target}|`;
      }
      return "[";
    },
    close: (_state, mark) => {
      const hrefRaw = mark.attrs.href as string;
      if (hrefRaw.startsWith("wiki:")) {
        return "]]";
      }
      const href = hrefRaw.replace(/[()]/g, "\\$&");
      const title = mark.attrs.title
        ? ` "${(mark.attrs.title as string).replace(/"/g, '\\"')}"`
        : "";
      return `](${href}${title})`;
    },
    mixable: true,
  },
};

/**
 * Renders one table cell's block content to a single inline line, since
 * pipe-table syntax has no representation for block structure within a
 * cell (Tiptap's own markdown docs note the same one-child-per-cell limit).
 * v0 tables are expected to hold a single paragraph per cell.
 */
function renderCellText(cell: PMNode): string {
  const rendered = serializer.serialize(cell, { tightLists: true });
  return rendered.replace(/\r?\n+/g, " ").replace(/\|/g, "\\|").trim();
}

function alignMarker(align: string | null): string {
  switch (align) {
    case "left":
      return ":--";
    case "center":
      return ":-:";
    case "right":
      return "--:";
    default:
      return "---";
  }
}

function serializeTable(state: MarkdownSerializerState, table: PMNode): void {
  const rows: string[][] = [];
  const aligns: (string | null)[] = [];
  table.forEach((row, _offset, rowIndex) => {
    const cells: string[] = [];
    row.forEach((cell, _cellOffset, colIndex) => {
      if (rowIndex === 0) aligns[colIndex] = (cell.attrs.align as string | null) ?? null;
      cells.push(renderCellText(cell));
    });
    rows.push(cells);
  });

  const columnCount = rows.reduce((max, row) => Math.max(max, row.length), 0);
  rows.forEach((cells, rowIndex) => {
    const padded = Array.from({ length: columnCount }, (_, i) => cells[i] ?? "");
    state.write(`| ${padded.join(" | ")} |\n`);
    if (rowIndex === 0) {
      const markers = Array.from({ length: columnCount }, (_, i) => alignMarker(aligns[i] ?? null));
      state.write(`| ${markers.join(" | ")} |\n`);
    }
  });
  state.closeBlock(table);
}

const serializerNodes: MarkdownSerializer["nodes"] = {
  paragraph(state, node) {
    state.renderInline(node);
    state.closeBlock(node);
  },
  blockquote(state, node) {
    state.wrapBlock("> ", null, node, () => state.renderContent(node));
  },
  codeBlock(state, node) {
    const backtickRuns = node.textContent.match(/`{3,}/gm);
    const fence = backtickRuns ? backtickRuns.sort().slice(-1)[0] + "`" : "```";
    state.write(fence + (node.attrs.language || "") + "\n");
    state.text(node.textContent, false);
    state.write("\n");
    state.write(fence);
    state.closeBlock(node);
  },
  heading(state, node) {
    state.write(`${state.repeat("#", node.attrs.level as number)} `);
    state.renderInline(node, false);
    state.closeBlock(node);
  },
  horizontalRule(state, node) {
    state.write("---");
    state.closeBlock(node);
  },
  bulletList(state, node) {
    state.renderList(node, "  ", () => "- ");
  },
  orderedList(state, node) {
    const start = (node.attrs.start as number) ?? 1;
    const maxWidth = String(start + node.childCount - 1).length;
    const space = state.repeat(" ", maxWidth + 2);
    state.renderList(node, space, (i) => {
      const marker = String(start + i);
      return state.repeat(" ", maxWidth - marker.length) + marker + ". ";
    });
  },
  listItem(state, node) {
    state.renderContent(node);
  },
  hardBreak(state, node, parent, index) {
    for (let i = index + 1; i < parent.childCount; i++) {
      if (parent.child(i).type !== node.type) {
        state.write("\\\n");
        return;
      }
    }
  },
  image(state, node) {
    const alt = state.esc(node.attrs.alt ?? "");
    const src = (node.attrs.src as string).replace(/[()]/g, "\\$&");
    const title = node.attrs.title
      ? ` "${(node.attrs.title as string).replace(/"/g, '\\"')}"`
      : "";
    state.write(`![${alt}](${src}${title})`);
  },
  table: serializeTable,
  text(state, node) {
    state.text(node.text ?? "");
  },
};

const serializer = new MarkdownSerializer(serializerNodes, serializerMarks);

/** Serialize a Tiptap JSON document back to a page body (no frontmatter). */
export function serializeJSONToMarkdown(json: JSONContent): string {
  const node = schema.nodeFromJSON(json);
  let markdown = serializer.serialize(node, { tightLists: true });
  // Collapse `[[Target|Target]]` produced when the wiki label matches the target.
  markdown = markdown.replace(/\[\[([^\]|]+)\|\1\]\]/g, "[[$1]]");
  if (!markdown.endsWith("\n")) markdown += "\n";
  return markdown;
}

// ---------------------------------------------------------------------------
// Frontmatter: kept verbatim, never parsed or edited in v0
// ---------------------------------------------------------------------------

const FRONTMATTER_PATTERN = /^---\r?\n(?:[\s\S]*?\r?\n)?(?:---|\.\.\.)[ \t]*\r?\n?/;

export interface SplitDocument {
  /** The frontmatter block, fences included verbatim, or `null` if absent. */
  frontmatter: string | null;
  /** Everything after the frontmatter block (the editable page body). */
  body: string;
}

/** Split a page's raw file content into its frontmatter block and body. */
export function splitFrontmatter(raw: string): SplitDocument {
  const match = FRONTMATTER_PATTERN.exec(raw);
  if (!match) return { frontmatter: null, body: raw };
  return { frontmatter: match[0], body: raw.slice(match[0].length) };
}

/** Reassemble a page's raw file content from its frontmatter and body. */
export function joinFrontmatter(frontmatter: string | null, body: string): string {
  return frontmatter ? frontmatter + body : body;
}
