import { createContext, useContext } from "react";

export const SettingsHighlightContext = createContext<string | null>(null);

export function useSettingsHighlight(settingId?: string): boolean {
  const highlightedId = useContext(SettingsHighlightContext);
  return settingId != null && settingId === highlightedId;
}

export function settingRowClassName(settingId: string | undefined, highlightedId: string | null) {
  const highlighted = settingId != null && settingId === highlightedId;
  return `setting-row${highlighted ? " setting-row-highlight" : ""}`;
}
