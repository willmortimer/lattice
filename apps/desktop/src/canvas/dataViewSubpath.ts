/** Map JSON Canvas file-node subpaths onto data-app open targets. */

/** Map a JSON Canvas file-node subpath to a data-app view name. */
export function viewNameFromCanvasSubpath(subpath: string | undefined): string | null {
  if (!subpath) return null;
  const normalized = normalizeSubpath(subpath);
  if (!normalized) return null;

  const withYaml = /^views\/([^/]+?)(?:\.view)?\.yaml$/i.exec(normalized);
  if (withYaml) return withYaml[1]!;

  const bare = /^views\/([^/]+)$/i.exec(normalized);
  if (bare) return bare[1]!;

  return null;
}

/**
 * Map a JSON Canvas file-node subpath to a package interface name.
 *
 * Choice: extend the existing `subpath` convention (same as `views/…`) rather
 * than inventing a separate canvas field. Recognized forms:
 * - `interfaces/{name}`
 * - `interfaces/{name}.interface.yaml`
 */
export function interfaceNameFromCanvasSubpath(subpath: string | undefined): string | null {
  if (!subpath) return null;
  const normalized = normalizeSubpath(subpath);
  if (!normalized) return null;

  const withSuffix = /^interfaces\/([^/]+)\.interface\.yaml$/i.exec(normalized);
  if (withSuffix) return withSuffix[1]!;

  // Bare stem only — reject accidental `*.yaml` without the `.interface` suffix.
  const bare = /^interfaces\/([^/.]+)$/i.exec(normalized);
  if (bare) return bare[1]!;

  return null;
}

/** Minimal interface binding shape used by canvas open resolution. */
export interface InterfaceOpenBindings {
  views: string[];
  forms: string[];
}

/**
 * Canvas open resolves an interface to its primary bound view (first `views`
 * entry). Form-only interfaces return null so the package opens on its default
 * view; package forms remain available via the Forms panel.
 */
export function viewNameFromInterfaceBindings(
  iface: InterfaceOpenBindings,
): string | null {
  const primary = iface.views[0]?.trim();
  return primary ? primary : null;
}

function normalizeSubpath(subpath: string): string {
  return subpath.replace(/\\/g, "/").replace(/^\/+/, "").trim();
}
