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
 * Resolve a page-relative embed path against the page's own directory. The
 * resulting path remains workspace-relative and is passed to the validated
 * `read_binary_file` command; it is never concatenated into a filesystem URL.
 */
export function resolveWorkspaceAssetPath(pagePath: string, src: string): string | null {
  if (isAbsoluteSrc(src)) return null;
  const pageDir = pagePath.includes("/") ? pagePath.slice(0, pagePath.lastIndexOf("/")) : "";
  return joinRelativePath(pageDir, src);
}

/** Infer a useful Blob MIME type from a workspace-relative filename. */
export function assetMimeType(path: string): string {
  const extension = path.split(".").pop()?.toLowerCase();
  switch (extension) {
    case "avif":
      return "image/avif";
    case "gif":
      return "image/gif";
    case "jpeg":
    case "jpg":
      return "image/jpeg";
    case "png":
      return "image/png";
    case "svg":
      return "image/svg+xml";
    case "webp":
      return "image/webp";
    default:
      return "application/octet-stream";
  }
}
