/**
 * Maps Markdown fence language tags to Shiki language ids.
 * Unknown / empty tags fall back to plain `text` (no grammar).
 */
const FENCE_LANGUAGE_ALIASES: Record<string, string> = {
  ts: "typescript",
  typescript: "typescript",
  js: "javascript",
  javascript: "javascript",
  rust: "rust",
  py: "python",
  python: "python",
  json: "json",
  yaml: "yaml",
  yml: "yaml",
  bash: "bash",
  sh: "bash",
  shell: "bash",
  sql: "sql",
};

/** Shiki id used when the fence language is missing or unsupported. */
export const PLAIN_FENCE_LANGUAGE = "text";

/**
 * Normalize a fence `language` attribute to a Shiki language id.
 * Mermaid is intentionally not mapped here — callers skip highlighting for it.
 */
export function mapFenceLanguage(language: string | null | undefined): string {
  if (language == null) return PLAIN_FENCE_LANGUAGE;
  const key = language.trim().toLowerCase();
  if (key.length === 0) return PLAIN_FENCE_LANGUAGE;
  return FENCE_LANGUAGE_ALIASES[key] ?? PLAIN_FENCE_LANGUAGE;
}
