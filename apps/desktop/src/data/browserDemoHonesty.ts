/** Shared copy for controls unavailable in the Vite browser demo fixture. */

export const NATIVE_DESKTOP_LABEL = "Native desktop";

const NATIVE_ONLY_TOOLTIP_SUFFIX =
  "Open this workspace with nxr desktop-dev or the installed Lattice.app.";

export function nativeOnlyToolbarTooltip(feature: string): string {
  return `${feature} requires the native desktop app. ${NATIVE_ONLY_TOOLTIP_SUFFIX}`;
}

export function isDataBrowserDemo(demoMutate: unknown): boolean {
  return Boolean(demoMutate);
}

export function nativeOnlyDemoNotice(feature: string): string {
  return `${feature} requires the ${NATIVE_DESKTOP_LABEL.toLowerCase()} app. Changes are not persisted in the browser demo.`;
}
