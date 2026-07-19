import { useEffect, useRef, useState } from "react";

/** Default beat on the branded splash before revealing the shell. */
export const DEFAULT_STARTUP_SPLASH_MS = 1000;

/** Hard cap so a failed theme load cannot trap the splash forever. */
export const STARTUP_SPLASH_MAX_MS = 3000;

export interface StartupSplashGateInput {
  enabled: boolean;
  profileReady: boolean;
  themeReady: boolean;
  /** Elapsed ms since splash mount. */
  elapsedMs: number;
  minMs?: number;
  maxMs?: number;
}

/**
 * Remaining hold after readiness, or `null` while still waiting on profile/theme
 * (unless the max wait has elapsed). `0` means dismiss now.
 */
export function startupSplashRemainingMs({
  enabled,
  profileReady,
  themeReady,
  elapsedMs,
  minMs = DEFAULT_STARTUP_SPLASH_MS,
  maxMs = STARTUP_SPLASH_MAX_MS,
}: StartupSplashGateInput): number | null {
  if (elapsedMs >= maxMs) return 0;
  if (!profileReady || !themeReady) return null;
  if (!enabled) return 0;
  return Math.max(0, minMs - elapsedMs);
}

export interface UseStartupSplashOptions {
  /** From workspace startup settings; false skips the intentional hold. */
  enabled: boolean;
  profileReady: boolean;
  themeReady: boolean;
  minMs?: number;
  maxMs?: number;
}

/**
 * Holds a branded splash until profile + theme are ready, and (when enabled)
 * until a short minimum duration has elapsed so startup does not flash past it.
 */
export function useStartupSplash({
  enabled,
  profileReady,
  themeReady,
  minMs = DEFAULT_STARTUP_SPLASH_MS,
  maxMs = STARTUP_SPLASH_MAX_MS,
}: UseStartupSplashOptions): boolean {
  const startedAtRef = useRef(typeof performance !== "undefined" ? performance.now() : Date.now());
  const [visible, setVisible] = useState(true);

  useEffect(() => {
    const now = () => (typeof performance !== "undefined" ? performance.now() : Date.now());
    const remaining = startupSplashRemainingMs({
      enabled,
      profileReady,
      themeReady,
      elapsedMs: now() - startedAtRef.current,
      minMs,
      maxMs,
    });
    if (remaining === null) {
      const untilMax = Math.max(0, maxMs - (now() - startedAtRef.current));
      const timer = window.setTimeout(() => setVisible(false), untilMax);
      return () => window.clearTimeout(timer);
    }
    if (remaining === 0) {
      setVisible(false);
      return;
    }
    const timer = window.setTimeout(() => setVisible(false), remaining);
    return () => window.clearTimeout(timer);
  }, [enabled, maxMs, minMs, profileReady, themeReady]);

  return visible;
}
