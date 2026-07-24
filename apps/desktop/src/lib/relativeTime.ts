/**
 * Human time formatting shared by the resource surfaces (workflow, task,
 * derived, artifact). Relative strings stay short ("4m ago"); pair them with
 * `formatAbsoluteTime` in a `title` attribute so the exact instant is one
 * hover away.
 */

const MINUTE = 60_000;
const HOUR = 3_600_000;
const DAY = 86_400_000;

/** "just now" / "4m ago" / "3h ago" / "2d ago", falling back to a date. */
export function formatRelativeTime(iso: string, now: number = Date.now()): string {
  const then = Date.parse(iso);
  if (Number.isNaN(then)) return iso;
  const delta = now - then;
  if (delta < 0) return "just now";
  if (delta < 45_000) return "just now";
  if (delta < HOUR) return `${Math.max(1, Math.round(delta / MINUTE))}m ago`;
  if (delta < DAY) return `${Math.round(delta / HOUR)}h ago`;
  if (delta < 14 * DAY) return `${Math.round(delta / DAY)}d ago`;
  return new Date(then).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

/** Full local timestamp for `title` attributes and detail rows. */
export function formatAbsoluteTime(iso: string): string {
  const then = Date.parse(iso);
  if (Number.isNaN(then)) return iso;
  return new Date(then).toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

/** Elapsed time between two ISO instants ("312ms", "4.2s", "2m 05s"). */
export function formatDurationBetween(
  startedAt: string,
  finishedAt?: string,
  now: number = Date.now(),
): string | null {
  const start = Date.parse(startedAt);
  if (Number.isNaN(start)) return null;
  const end = finishedAt ? Date.parse(finishedAt) : now;
  if (Number.isNaN(end)) return null;
  const ms = Math.max(0, end - start);
  if (ms < 1000) return `${ms}ms`;
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  const minutes = Math.floor(seconds / 60);
  const rem = Math.round(seconds - minutes * 60);
  return `${minutes}m ${String(rem).padStart(2, "0")}s`;
}

/** Humanize a plain seconds count ("45s", "15 min", "2 h", "1 day"). */
export function formatSeconds(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return `${seconds}s`;
  if (seconds < 60) return `${seconds}s`;
  if (seconds % 3600 === 0) {
    const hours = seconds / 3600;
    if (hours % 24 === 0) {
      const days = hours / 24;
      return days === 1 ? "1 day" : `${days} days`;
    }
    return hours === 1 ? "1 h" : `${hours} h`;
  }
  if (seconds % 60 === 0) return `${seconds / 60} min`;
  const minutes = Math.floor(seconds / 60);
  return `${minutes} min ${seconds % 60}s`;
}
