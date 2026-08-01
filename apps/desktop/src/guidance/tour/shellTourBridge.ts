const SHELL_TOUR_START_EVENT = "lattice:shell-tour-start";

/** Request the workspace shell quick-start tour (handled by GuidanceTourController). */
export function requestShellTourStart(): void {
  window.dispatchEvent(new CustomEvent(SHELL_TOUR_START_EVENT));
}

export function subscribeShellTourStart(listener: () => void): () => void {
  const handler = () => listener();
  window.addEventListener(SHELL_TOUR_START_EVENT, handler);
  return () => window.removeEventListener(SHELL_TOUR_START_EVENT, handler);
}
