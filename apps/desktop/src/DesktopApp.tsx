import { QueryClientProvider } from "@tanstack/react-query";

import { useDesktopController } from "./controllers/useDesktopController";
import { queryClient } from "./query/queryClient";
import { DesktopShell } from "./shell/DesktopShell";
import { DesktopUiStoreProvider } from "./shell/desktopUiStore";

function DesktopAppInner() {
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
