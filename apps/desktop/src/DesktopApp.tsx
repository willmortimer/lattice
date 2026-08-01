import { QueryClientProvider } from "@tanstack/react-query";
import { useEffect } from "react";

import { useDesktopController } from "./controllers/useDesktopController";
import { registerBrowserPerfHarness } from "./dev/perfHarness";
import { queryClient } from "./query/queryClient";
import { DesktopShell } from "./shell/DesktopShell";
import { DesktopUiStoreProvider, useDesktopUiStoreApi } from "./shell/desktopUiStore";

function DesktopAppInner() {
  const uiStore = useDesktopUiStoreApi();

  useEffect(() => {
    registerBrowserPerfHarness(uiStore);
  }, [uiStore]);

  return <DesktopShell model={useDesktopController()} />;
}

export default function DesktopApp() {
  return (
    <QueryClientProvider client={queryClient}>
      <DesktopUiStoreProvider>
        <DesktopAppInner />
      </DesktopUiStoreProvider>
    </QueryClientProvider>
  );
}
