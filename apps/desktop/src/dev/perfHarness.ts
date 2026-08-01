import { inBrowser } from "../demo";
import type { DesktopUiStore } from "../shell/desktopUiStore";

export type LatticePerfHarness = {
  version: 1;
  /** Open the agent panel in workbench layout (browser demo fixture). */
  prepareAgentWorkbench: () => void;
  /** Whether `injectAgentMessages` can seed a transcript for perf smokes. */
  canInjectAgentMessages: () => boolean;
  /** Seed synthetic transcript rows; returns false when unsupported. */
  injectAgentMessages: (count: number) => Promise<boolean>;
};

declare global {
  interface Window {
    __latticePerfHarness?: LatticePerfHarness;
  }
}

/**
 * Dev-only Playwright bridge for agent/workbench perf stubs.
 * Browser demo cannot hydrate a real agent thread — injection stays false there.
 */
export function registerBrowserPerfHarness(uiStore: DesktopUiStore): void {
  if (!inBrowser) {
    return;
  }

  window.__latticePerfHarness = {
    version: 1,
    prepareAgentWorkbench: () => {
      const state = uiStore.getState();
      state.setAgentPanelOpen(true);
      state.setAgentLayoutMode("workbench");
    },
    canInjectAgentMessages: () => false,
    injectAgentMessages: async () => false,
  };
}
