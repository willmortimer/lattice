/**
 * Host-owned document assembly for script-free HTML previews.
 *
 * This is intentionally a small allow-by-removal boundary, paired with a
 * bare iframe sandbox and a no-network CSP. It is shared by artifact previews
 * and ordinary HTML files so neither path accidentally grows a privileged
 * iframe bridge.
 */
import DOMPurify from "dompurify";

export const STATIC_DOCUMENT_CSP = [
  "default-src 'none'",
  "script-src 'none'",
  "connect-src 'none'",
  "frame-src 'none'",
  "object-src 'none'",
  "form-action 'none'",
  "base-uri 'none'",
  "img-src data:",
  "font-src data:",
  "style-src 'unsafe-inline'",
].join("; ");

export const LATTICE_STATIC_CSS = `
:root { color-scheme: light dark; font-family: var(--lt-font-ui, system-ui, sans-serif); color: var(--lt-text, #18212f); background: var(--lt-bg, #f5f7fb); }
* { box-sizing: border-box; } body { margin: 0; padding: 1.25rem; background: var(--lt-bg, #f5f7fb); color: var(--lt-text, #18212f); line-height: 1.5; }
a { color: var(--lt-accent, #a85e00); } button, .lt-button { border: 1px solid var(--lt-border, #c7ceda); border-radius: var(--lt-radius-sm, 6px); padding: .45rem .75rem; background: var(--lt-panel, #fff); color: inherit; font: inherit; }
.lt-document, .lt-container { width: min(100%, 72rem); margin-inline: auto; }.lt-stack { display: grid; gap: var(--lt-space, 1rem); }.lt-cluster { display:flex; flex-wrap:wrap; gap:.75rem; align-items:center; }.lt-grid { display:grid; grid-template-columns:repeat(auto-fit,minmax(14rem,1fr)); gap:1rem; }.lt-split { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:1rem; }.lt-scroll { overflow:auto; max-block-size:32rem; }.lt-prose { max-width:72ch; }.lt-card,.lt-callout,.lt-empty,.lt-degraded { border:1px solid var(--lt-border,#c7ceda); border-radius:var(--lt-radius,10px); padding:1rem; background:var(--lt-panel,#fff); }.lt-callout { border-inline-start:4px solid var(--lt-accent,#a85e00); }.lt-stat { font-size:2rem; font-weight:700; }.lt-badge { display:inline-flex; border:1px solid var(--lt-border,#c7ceda); border-radius:999px; padding:.1rem .5rem; font-size:.8rem; }.lt-table { width:100%; border-collapse:collapse; }.lt-table :is(th,td) { border-bottom:1px solid var(--lt-border,#c7ceda); padding:.5rem; text-align:start; } @media (max-width: 40rem) { .lt-split { grid-template-columns:1fr; } }
`;

const BLOCKED_TAGS = ["script", "iframe", "frame", "frameset", "object", "embed", "applet", "form", "base", "meta", "link", "svg", "math"];

/** Strip CSS mechanisms that can fetch assets outside the package boundary. */
export function sanitizeStaticCss(css: string): string {
  // CSS URL loading and @import would escape the static package authority.
  return css.replace(/@import[^;]+;?/gi, "").replace(/url\s*\([^)]*\)/gi, "none");
}

function themeCss(vars: Record<string, string> | undefined): string {
  if (!vars) return "";
  const declarations = Object.entries(vars)
    .filter(([name, value]) => name.startsWith("--lt-") && !/[{};]/.test(value))
    .map(([name, value]) => `${name}:${value}`)
    .join(";");
  return declarations ? `:root{${declarations}}` : "";
}

/** Returns an inert standalone document suitable only for `sandbox=""`. */
export function assembleStaticDocument(input: {
  html: string;
  title?: string | null;
  styles?: string[];
  /** Package-local, size-bounded raster assets keyed by relative path. */
  assets?: Record<string, string>;
  themeVars?: Record<string, string>;
  includeVocabulary?: boolean;
}): string {
  const clean = DOMPurify.sanitize(input.html, {
    WHOLE_DOCUMENT: true,
    FORBID_TAGS: BLOCKED_TAGS,
    FORBID_ATTR: ["style"],
    ALLOW_DATA_ATTR: true,
  });
  const parser = new DOMParser();
  const doc = parser.parseFromString(clean, "text/html");
  doc.querySelectorAll(BLOCKED_TAGS.join(",")).forEach((node) => node.remove());
  doc.querySelectorAll("*").forEach((element) => {
    for (const attribute of [...element.attributes]) {
      const name = attribute.name.toLowerCase();
      const value = attribute.value.trim();
      if (name.startsWith("on") || name === "srcdoc" || name === "action" || name === "formaction") element.removeAttribute(attribute.name);
      if (name === "src" && element.tagName.toLowerCase() === "img") {
        const replacement = input.assets?.[value];
        if (replacement) element.setAttribute(attribute.name, replacement);
        else if (!value.startsWith("data:image/")) element.removeAttribute(attribute.name);
      } else if (["href", "src", "xlink:href"].includes(name) && !value.startsWith("data:image/")) {
        element.removeAttribute(attribute.name);
      }
    }
  });
  const title = input.title ?? doc.title ?? "Lattice static document";
  doc.title = title;
  const head = doc.head || doc.documentElement.insertBefore(doc.createElement("head"), doc.body);
  head.querySelectorAll("meta[http-equiv], meta[charset]").forEach((node) => node.remove());
  const charset = doc.createElement("meta"); charset.setAttribute("charset", "utf-8"); head.prepend(charset);
  const csp = doc.createElement("meta"); csp.httpEquiv = "Content-Security-Policy"; csp.content = STATIC_DOCUMENT_CSP; head.prepend(csp);
  const styles = doc.createElement("style");
  styles.textContent = `${themeCss(input.themeVars)}${input.includeVocabulary !== false ? LATTICE_STATIC_CSS : ""}${(input.styles ?? []).map(sanitizeStaticCss).join("\n")}`;
  head.append(styles);
  return `<!doctype html>\n${doc.documentElement.outerHTML}`;
}
