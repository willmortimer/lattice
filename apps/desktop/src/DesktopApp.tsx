import { DesktopShell } from "./shell/DesktopShell";
import { DesktopUiStoreProvider } from "./shell/desktopUiStore";
import { useDesktopController } from "./controllers/useDesktopController";

function DesktopAppInner() {
  return <DesktopShell model={useDesktopController()} />;
}

export default function DesktopApp() {
  return (
    <DesktopUiStoreProvider>
      <DesktopAppInner />
    </DesktopUiStoreProvider>
  );
}
