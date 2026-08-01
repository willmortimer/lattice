import {
  rendererSessionIdForPath,
  saveStatusForSession,
  useDesktopUiStore,
  type RendererSessionId,
} from "./desktopUiStore";
import { isUnsaved, saveIndicatorText } from "../editor/saveState";

/** Save-status chrome that subscribes narrowly — typing does not rerender DesktopShell. */
export function SaveStatusIndicator({
  sessionId,
  externalConflict,
}: {
  sessionId: RendererSessionId | null;
  externalConflict: boolean;
}) {
  const saveState = useDesktopUiStore((state) =>
    saveStatusForSession(state.saveStatusBySessionId, sessionId),
  );
  return (
    <span className={`save-state save-state-${saveState.status}`}>
      {externalConflict ? "Conflict" : saveIndicatorText(saveState) || "Saved"}
    </span>
  );
}

/** Tab chrome: each tab subscribes to its own renderer session save status. */
export function TabUnsavedDot({ path }: { path: string }) {
  const sessionId = rendererSessionIdForPath(path);
  const unsaved = useDesktopUiStore((state) =>
    isUnsaved(saveStatusForSession(state.saveStatusBySessionId, sessionId)),
  );
  if (!unsaved) return null;
  return <i />;
}
