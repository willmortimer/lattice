/**
 * Filesystem-safe timestamp (colons and the decimal point replaced with
 * `-`) for generated filenames — millisecond precision keeps repeated
 * presses of the same shortcut from colliding.
 */
export function fileTimestamp(now: Date = new Date()): string {
  return now.toISOString().replace(/[:.]/g, "-");
}

/** The quick-note path (docs/07's "Quick-note mode"): a new page in the configured folder. */
export function quickNotePath(now: Date = new Date(), directory = "Inbox"): string {
  const normalized = directory.trim().replace(/^\/+|\/+$/g, "") || "Inbox";
  return `${normalized}/${fileTimestamp(now)}.md`;
}
