/** Map a JSON Canvas file-node subpath to a data-app view name. */
export function viewNameFromCanvasSubpath(subpath: string | undefined): string | null {
  if (!subpath) return null;
  const normalized = subpath.replace(/\\/g, "/").replace(/^\/+/, "").trim();
  if (!normalized) return null;

  const withYaml = /^views\/([^/]+?)(?:\.view)?\.yaml$/i.exec(normalized);
  if (withYaml) return withYaml[1]!;

  const bare = /^views\/([^/]+)$/i.exec(normalized);
  if (bare) return bare[1]!;

  return null;
}
