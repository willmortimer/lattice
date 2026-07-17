import createDOMPurify from "dompurify";

/** Strict allowlist for HTML/RTF→HTML paste. No scripts, handlers, styles, or remote media. */
const ALLOWED_TAGS = [
  "p",
  "br",
  "strong",
  "em",
  "b",
  "i",
  "s",
  "code",
  "pre",
  "blockquote",
  "ul",
  "ol",
  "li",
  "a",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "hr",
  "table",
  "thead",
  "tbody",
  "tr",
  "th",
  "td",
];

const ALLOWED_ATTR = ["href", "title", "colspan", "rowspan"];

/** Node-safe fallback used by unit tests and any non-DOM callers. */
function stripUnsafeHtml(html: string): string {
  return html
    .replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, "")
    .replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi, "")
    .replace(/<\/?(?:img|iframe|object|embed|form|input|video|audio|source)\b[^>]*>/gi, "")
    .replace(/\son\w+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, "")
    .replace(/\sstyle\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, "")
    .replace(/\s(?:src|srcset|poster)\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, "");
}

export function sanitizePastedHtml(html: string): string {
  if (typeof window === "undefined") {
    return stripUnsafeHtml(html);
  }
  const purify = createDOMPurify(window);
  return purify.sanitize(html, {
    ALLOWED_TAGS,
    ALLOWED_ATTR,
    ALLOW_DATA_ATTR: false,
    FORBID_TAGS: ["style", "script", "iframe", "object", "embed", "form", "input", "img", "video", "audio", "source"],
    FORBID_ATTR: ["style", "src", "srcset", "poster", "onerror", "onclick", "onload", "class", "id"],
  });
}

export type PasteKind = "files" | "markdown" | "html" | "plain" | "none";

/** Resolve paste priority: files → text/markdown → sanitized HTML → plain text. */
export function classifyClipboard(data: DataTransfer | null | undefined): PasteKind {
  if (!data) return "none";
  if (data.files && data.files.length > 0) return "files";
  if (data.getData("text/markdown").trim()) return "markdown";
  if (data.getData("text/html").trim()) return "html";
  if (data.getData("text/plain").trim()) return "plain";
  return "none";
}
