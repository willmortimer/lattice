import { QueryClientProvider } from "@tanstack/react-query";
import { useEffect } from "react";

import { useDesktopController } from "./controllers/useDesktopController";
import { seedGuidanceAnchors } from "./guidance";
import { queryClient } from "./query/queryClient";
import { DesktopShell } from "./shell/DesktopShell";
import { DesktopUiStoreProvider } from "./shell/desktopUiStore";

function DesktopAppInner() {
  useEffect(() => seedGuidanceAnchors(), []);
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
