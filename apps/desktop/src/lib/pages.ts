import { invoke } from "@tauri-apps/api/core";

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
