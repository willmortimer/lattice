/** Display label for the active resource path chip. */
export function resourcePathChipLabel(path: string | null | undefined): string | null {
  const trimmed = path?.trim();
  if (!trimmed) {
    return null;
  }
  const segments = trimmed.split("/").filter((segment) => segment.length > 0);
  return segments.at(-1) ?? trimmed;
}

/** Display label for the workspace root chip. */
export function workspaceChipLabel(root: string | null | undefined): string | null {
  const trimmed = root?.trim();
  if (!trimmed) {
    return null;
  }
  const normalized = trimmed.replace(/\/+$/, "");
  const segments = normalized.split("/").filter((segment) => segment.length > 0);
  return segments.at(-1) ?? normalized;
}
