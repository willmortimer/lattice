import { convertFileSrc } from "@tauri-apps/api/core";

/** `scheme:` or `//host` prefixes — anything that isn't a page-relative path. */
const ABSOLUTE_SRC_PATTERN = /^([a-z][a-z0-9+.-]*:|\/\/)/i;

/**
 * Whether `src` is already a fully-qualified reference (`https://…`,
 * `data:…`, a protocol-relative `//cdn…`, and so on) rather than a path
 * relative to the page it's embedded in.
 */
export function isAbsoluteSrc(src: string): boolean {
  return ABSOLUTE_SRC_PATTERN.test(src);
}

/**
 * Resolve `.`/`..` segments in `relative` against `baseDir` (both using
 * `/` separators), the way a browser resolves a relative URL against its
 * page. A `..` past the top of `baseDir` is simply dropped rather than
 * escaping above it.
 */
export function joinRelativePath(baseDir: string, relative: string): string {
  const parts = baseDir.split("/").filter((segment) => segment.length > 0);
  for (const part of relative.split("/")) {
    if (part === "" || part === ".") continue;
    if (part === "..") parts.pop();
    else parts.push(part);
  }
  return parts.join("/");
}

/**
 * Resolve a page-relative embed `src` (an image path from Markdown) to a
 * URL the webview can actually load: the Tauri asset protocol, scoped to
 * `root` and resolved relative to `pagePath`'s directory (not the
 * workspace root — Markdown image paths are relative to the file that
 * contains them).
 *
 * Returns `src` unchanged when it's already absolute, or when there is no
 * workspace `root` to resolve against (the in-browser demo shell has no
 * real files on disk, and no Tauri bridge to ask).
 */
export function resolveEmbedSrc(root: string | null, pagePath: string, src: string): string {
  if (!root || isAbsoluteSrc(src)) return src;
  const pageDir = pagePath.includes("/") ? pagePath.slice(0, pagePath.lastIndexOf("/")) : "";
  const relPath = joinRelativePath(pageDir, src);
  return convertFileSrc(`${root}/${relPath}`);
}
