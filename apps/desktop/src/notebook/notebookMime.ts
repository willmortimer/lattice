import createDOMPurify from "dompurify";

export interface NotebookDisplayData {
  textPlain?: string;
  imageDataUrl?: string;
  html?: string;
  markdown?: string;
  svg?: string;
  vegaLite?: Record<string, unknown>;
}

/** Jupyter Vega-Lite MIME keys (`application/vnd.vegalite.v5+json`, etc.). */
export const VEGA_LITE_MIME_PATTERN = /^application\/vnd\.vegalite\.v\d+(?:\.\d+)?\+json$/;

const IMAGE_MIME_PREFIXES: ReadonlyArray<readonly [string, string]> = [
  ["image/png", "data:image/png;base64,"],
  ["image/jpeg", "data:image/jpeg;base64,"],
  ["image/jpg", "data:image/jpeg;base64,"],
  ["image/gif", "data:image/gif;base64,"],
  ["image/webp", "data:image/webp;base64,"],
];

const NOTEBOOK_HTML_TAGS = [
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
  "img",
  "div",
  "span",
  "sup",
  "sub",
];

const NOTEBOOK_HTML_ATTR = ["href", "title", "colspan", "rowspan", "src", "alt", "width", "height"];

function joinMultiline(value: unknown): string {
  if (typeof value === "string") return value;
  if (Array.isArray(value)) return value.map((entry) => String(entry)).join("");
  return "";
}

function parseVegaLite(value: unknown): Record<string, unknown> | undefined {
  if (value !== null && typeof value === "object" && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  const text = joinMultiline(value);
  if (!text) return undefined;
  try {
    const parsed = JSON.parse(text) as unknown;
    if (parsed !== null && typeof parsed === "object" && !Array.isArray(parsed)) {
      return parsed as Record<string, unknown>;
    }
  } catch {
    return undefined;
  }
  return undefined;
}

function extractSvg(data: Record<string, unknown>): string | undefined {
  const raw = joinMultiline(data["image/svg+xml"]);
  if (!raw) return undefined;
  if (raw.trimStart().startsWith("<")) return raw;
  return undefined;
}

function extractImageDataUrl(data: Record<string, unknown>): string | undefined {
  for (const [mime, prefix] of IMAGE_MIME_PREFIXES) {
    const encoded = data[mime];
    if (typeof encoded === "string" && encoded.length > 0) {
      return `${prefix}${encoded}`;
    }
  }
  const svg = joinMultiline(data["image/svg+xml"]);
  if (svg && !svg.trimStart().startsWith("<")) {
    return `data:image/svg+xml;base64,${svg}`;
  }
  return undefined;
}

function extractVegaLite(data: Record<string, unknown>): Record<string, unknown> | undefined {
  for (const [mime, value] of Object.entries(data)) {
    if (VEGA_LITE_MIME_PATTERN.test(mime)) {
      const parsed = parseVegaLite(value);
      if (parsed) return parsed;
    }
  }
  return undefined;
}

/** Parse a Jupyter MIME bundle into the notebook display read-model. */
export function mimeBundleToDisplayData(data: Record<string, unknown>): NotebookDisplayData {
  const textPlain = joinMultiline(data["text/plain"]);
  const markdown = joinMultiline(data["text/markdown"]);
  const html = joinMultiline(data["text/html"]);
  const svg = extractSvg(data);
  const imageDataUrl = extractImageDataUrl(data);
  const vegaLite = extractVegaLite(data);

  const result: NotebookDisplayData = {};
  if (textPlain) result.textPlain = textPlain;
  if (markdown) result.markdown = markdown;
  if (html) result.html = html;
  if (svg) result.svg = svg;
  if (imageDataUrl) result.imageDataUrl = imageDataUrl;
  if (vegaLite) result.vegaLite = vegaLite;
  return result;
}

function imageDataUrlToMime(imageDataUrl: string): Record<string, unknown> | null {
  for (const [mime, prefix] of IMAGE_MIME_PREFIXES) {
    if (imageDataUrl.startsWith(prefix)) {
      return { [mime]: imageDataUrl.slice(prefix.length) };
    }
  }
  if (imageDataUrl.startsWith("data:image/svg+xml;base64,")) {
    return {
      "image/svg+xml": imageDataUrl.slice("data:image/svg+xml;base64,".length),
    };
  }
  return null;
}

/** Serialize display read-model fields back to nbformat MIME keys. */
export function displayDataToMime(
  data: NotebookDisplayData,
  splitLines: (text: string) => string[],
): Record<string, unknown> {
  const mime: Record<string, unknown> = {};
  if (data.textPlain) mime["text/plain"] = splitLines(data.textPlain);
  if (data.markdown) mime["text/markdown"] = splitLines(data.markdown);
  if (data.html) mime["text/html"] = splitLines(data.html);
  if (data.svg) mime["image/svg+xml"] = splitLines(data.svg);
  if (data.vegaLite) mime["application/vnd.vegalite.v5+json"] = data.vegaLite;
  if (data.imageDataUrl) {
    const imageMime = imageDataUrlToMime(data.imageDataUrl);
    if (imageMime) Object.assign(mime, imageMime);
  }
  return mime;
}

function stripUnsafeNotebookHtml(html: string): string {
  return html
    .replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, "")
    .replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi, "")
    .replace(/<\/?(?:iframe|object|embed|form|input|video|audio|source)\b[^>]*>/gi, "")
    .replace(/\son\w+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, "")
    .replace(/\sstyle\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, "")
    .replace(/\ssrcset\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, "");
}

/** Sanitize notebook HTML output (tables/images with data URLs allowed). */
export function sanitizeNotebookHtml(html: string): string {
  if (typeof window === "undefined") {
    return stripUnsafeNotebookHtml(html);
  }
  const purify = createDOMPurify(window);
  return purify.sanitize(html, {
    ALLOWED_TAGS: NOTEBOOK_HTML_TAGS,
    ALLOWED_ATTR: NOTEBOOK_HTML_ATTR,
    ALLOW_DATA_ATTR: false,
    FORBID_TAGS: ["style", "script", "iframe", "object", "embed", "form", "input", "video", "audio", "source"],
    FORBID_ATTR: ["style", "srcset", "poster", "onerror", "onclick", "onload", "class", "id"],
    ALLOWED_URI_REGEXP: /^(?:(?:https?|data):|[^a-z]|[a-z+.\-]+(?:[^a-z+.\-:]|$))/i,
  });
}

/** Sanitize inline SVG for notebook output. */
export function sanitizeNotebookSvg(svg: string): string {
  if (typeof window === "undefined") {
    return stripUnsafeNotebookHtml(svg);
  }
  const purify = createDOMPurify(window);
  return purify.sanitize(svg, {
    USE_PROFILES: { svg: true, svgFilters: true },
    FORBID_TAGS: ["script", "foreignObject"],
  });
}
