import { useDesktopUiStore } from "./desktopUiStore";
import { isUnsaved, saveIndicatorText } from "../editor/saveState";

/** Save-status chrome that subscribes narrowly — typing does not rerender DesktopShell. */
export function SaveStatusIndicator({
  externalConflict,
}: {
  externalConflict: boolean;
}) {
  const saveState = useDesktopUiStore((state) => state.saveState);
  return (
    <span className={`save-state save-state-${saveState.status}`}>
      {externalConflict ? "Conflict" : saveIndicatorText(saveState) || "Saved"}
    </span>
  );
}

export function TabUnsavedDot({ active }: { active: boolean }) {
  const saveState = useDesktopUiStore((state) => state.saveState);
  if (!active || !isUnsaved(saveState)) return null;
  return <i />;
}
