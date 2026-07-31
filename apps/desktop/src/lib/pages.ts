import { invoke } from "./ipc";

export interface CreatePageInput {
  root: string;
  relPath: string;
  /** Used when `templatePath` is omitted (blank create). */
  content?: string;
  /** Workspace-relative Markdown template; Rust substitutes `{{title}}` / `{{date}}`. */
  templatePath?: string | null;
  /** Overrides the title derived from the page path stem. */
  title?: string | null;
}

/** Lean Quick Note bootstrap payload (Tauri-only; no full resource scan). */
export interface QuickNotePrepared {
  root: string;
  workspaceTitle: string;
  path: string;
  content: string;
  revision: string;
  quickNoteDirectory: string;
  templatePath?: string | null;
}

/**
 * Create a page through the semantic command core.
 *
 * When `templatePath` is set, body content is read and substituted in Rust —
 * the shell must not write template bodies itself.
 */
export async function createPage(input: CreatePageInput): Promise<string> {
  return invoke<string>("create_page", {
    root: input.root,
    relPath: input.relPath,
    content: input.content ?? "",
    templatePath: input.templatePath ?? null,
    title: input.title ?? null,
  });
}

/**
 * Open a workspace session, create a Quick Note page, and return its initial
 * content without listing the full resource catalog (Tauri-only).
 */
export async function prepareQuickNote(root: string): Promise<QuickNotePrepared> {
  return invoke<QuickNotePrepared>("prepare_quick_note", { root });
}

/**
 * Quick Note default template path.
 *
 * Prefer `<templateDirectory>/Daily.md` when `templateDirectory` is set;
 * otherwise the convention path `Templates/Daily.md` when that resource exists.
 */
export function resolveQuickNoteTemplatePath(
  templateDirectory: string | null | undefined,
  resourcePaths: readonly string[],
): string | undefined {
  const present = new Set(resourcePaths);
  const candidates: string[] = [];
  const trimmed = templateDirectory?.trim().replace(/^\/+|\/+$/g, "") ?? "";
  if (trimmed) {
    candidates.push(`${trimmed}/Daily.md`);
  }
  if (!candidates.includes("Templates/Daily.md")) {
    candidates.push("Templates/Daily.md");
  }
  return candidates.find((path) => present.has(path));
}
