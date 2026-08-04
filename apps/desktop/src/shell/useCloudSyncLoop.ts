import { useEffect, useRef } from "react";

import { CloudSyncLoop, registerCloudSyncLoop } from "../lib/cloudSync";
import type { CatalogEntry } from "../lib/resourceCatalog";
import { useDesktopUiStoreApi } from "./desktopUiStore";

export function useCloudSyncLoop(
  workspaceRoot: string | null,
  catalog: ReadonlyMap<string, CatalogEntry>,
): void {
  const uiStore = useDesktopUiStoreApi();
  const loopRef = useRef<CloudSyncLoop | null>(null);

  useEffect(() => {
    const loop = new CloudSyncLoop({
      workspaceRoot,
      catalog,
      onSnapshot: (snapshot) => {
        uiStore.getState().setWorkspaceCloudSync(snapshot);
      },
      onSyncBadges: (badges) => {
        uiStore.getState().setSyncBadges(badges);
      },
    });
    loop.start();
    loop.attachSaveStatusSubscription(uiStore.subscribe);
    loopRef.current = loop;
    registerCloudSyncLoop(loop);

    return () => {
      registerCloudSyncLoop(null);
      loop.dispose();
      loopRef.current = null;
      uiStore.getState().clearSyncBadges();
      uiStore.getState().setWorkspaceCloudSync({
        phase: "idle",
        message: null,
        lastSyncedAt: null,
        conflictCount: 0,
        errorCount: 0,
        cloudWorkspaceId: null,
      });
    };
  }, [uiStore, workspaceRoot]);

  useEffect(() => {
    loopRef.current?.updateContext(workspaceRoot, catalog);
  }, [catalog, workspaceRoot]);
}
