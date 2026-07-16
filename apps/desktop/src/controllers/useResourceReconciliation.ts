import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { inBrowser } from "../demo";
import type { WorkspaceChangeEvent } from "../types";

/** Installs the native watcher bridge once and leaves reconciliation policy to
 * the resource/workspace controllers. This keeps Tauri lifecycle wiring out
 * of renderers and is a no-op for the browser fixture. */
export function useResourceReconciliation(
  onChange: (event: WorkspaceChangeEvent) => void | Promise<void>,
): void {
  useEffect(() => {
    if (inBrowser) return;
    let unlisten: (() => void) | undefined;
    void listen<WorkspaceChangeEvent>("workspace-changed", (event) => {
      void onChange(event.payload);
    }).then((stop) => {
      unlisten = stop;
    });
    return () => unlisten?.();
  }, [onChange]);
}
