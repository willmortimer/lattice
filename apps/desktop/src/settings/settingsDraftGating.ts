import type { SettingsSection } from "./settingsCatalog";

/**
 * Settings sections that keep edits as drafts until the user clicks Apply.
 *
 * MVP coverage:
 * - `files` — workspace-relative paths (Quick Note folder) plus related autosave
 *   and close-guard preferences in the same section.
 * - `workspaces` — default workspace path and startup session preferences.
 *
 * Other sections still persist immediately (debounced profile save or live
 * workspace manifest updates). Extend this list when adding draft gating.
 */
export const DRAFT_GATED_SECTIONS: readonly SettingsSection[] = ["files", "workspaces"];

export function isDraftGatedSection(section: SettingsSection): boolean {
  return DRAFT_GATED_SECTIONS.includes(section);
}
