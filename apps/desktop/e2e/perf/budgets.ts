/**
 * Soft performance budgets for the browser demo harness.
 *
 * Doc targets (docs/23-frontend-rendering-and-performance.md):
 *   warm shell visible in 300–500 ms on representative hardware.
 *
 * CI defaults are intentionally generous; tighten locally with env vars.
 */
export interface PerfBudgets {
  /** First navigation to visible workspace chrome. */
  shellColdMs: number;
  /** Reload / repeat visit to visible workspace chrome. */
  shellWarmMs: number;
  /** Select a page in the resource tree until editor content is visible. */
  pageOpenMs: number;
  /** Scroll the open page content pane (smoke; demo pages are short). */
  pageScrollMs: number;
}

/** Documented local targets — logged for comparison, not asserted by default. */
export const documentedTargets = {
  shellWarmMs: 500,
} as const;

function readBudget(name: string, fallback: number): number {
  const raw = process.env[name];
  if (raw === undefined || raw === "") return fallback;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${name} must be a positive number (got ${raw})`);
  }
  return parsed;
}

/** CI-friendly defaults; override per machine when profiling locally. */
export const perfBudgets: PerfBudgets = {
  shellColdMs: readBudget("LATTICE_PERF_SHELL_COLD_MS", 8_000),
  shellWarmMs: readBudget("LATTICE_PERF_SHELL_WARM_MS", 3_000),
  pageOpenMs: readBudget("LATTICE_PERF_PAGE_OPEN_MS", 4_000),
  pageScrollMs: readBudget("LATTICE_PERF_PAGE_SCROLL_MS", 2_000),
};

export interface NavigationMetrics {
  domContentLoadedMs: number | null;
  loadEventEndMs: number | null;
  performanceNowMs: number;
}

export async function readNavigationMetrics(
  evaluate: <T>(pageFunction: () => T) => Promise<T>,
): Promise<NavigationMetrics> {
  return evaluate(() => {
    const nav = performance.getEntriesByType("navigation")[0] as
      | PerformanceNavigationTiming
      | undefined;
    return {
      domContentLoadedMs: nav?.domContentLoadedEventEnd ?? null,
      loadEventEndMs: nav?.loadEventEnd ?? null,
      performanceNowMs: performance.now(),
    };
  });
}

export interface HeapSnapshot {
  usedJsHeapBytes: number;
  totalJsHeapBytes: number;
}
