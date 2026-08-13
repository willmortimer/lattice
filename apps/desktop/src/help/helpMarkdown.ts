import DOMPurify from "dompurify";
import MarkdownIt from "markdown-it";

const helpMarkdown = new MarkdownIt("commonmark", { html: false, linkify: true }).enable([
  "table",
  "strikethrough",
]);

/** Render help corpus markdown to sanitized HTML (headings, lists, tables, code, links). */
export function renderHelpMarkdownHtml(markdown: string): string {
  const source = markdown.trim();
  if (!source) return "";
  return DOMPurify.sanitize(helpMarkdown.render(source));
}
