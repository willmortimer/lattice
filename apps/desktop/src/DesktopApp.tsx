import { DesktopShell } from "./shell/DesktopShell";
import { useDesktopController } from "./controllers/useDesktopController";
export default function DesktopApp() { return <DesktopShell model={useDesktopController()} />; }

