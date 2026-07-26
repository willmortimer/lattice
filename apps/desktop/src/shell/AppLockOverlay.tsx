import { Button } from "@lattice/ui";
import { Lock } from "@phosphor-icons/react";
import { useState } from "react";

import { BrandMark } from "./BrandMark";
import { unlockApp } from "../lib/appLock";

interface AppLockOverlayProps {
  onUnlocked: () => void;
}

/** Full-window privacy gate; shell content beneath should be inert / aria-hidden. */
export function AppLockOverlay({ onUnlocked }: AppLockOverlayProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleUnlock() {
    setBusy(true);
    setError(null);
    try {
      const status = await unlockApp();
      if (!status.locked) {
        onUnlocked();
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="app-lock-overlay" role="dialog" aria-modal="true" aria-label="Lattice is locked">
      <BrandMark size={56} />
      <h1 className="empty-wordmark">Lattice</h1>
      <p className="app-lock-copy">Session locked. Unlock with Touch ID or your device password.</p>
      <Button type="button" onClick={() => void handleUnlock()} disabled={busy}>
        <Lock size={16} aria-hidden />
        {busy ? "Waiting…" : "Unlock"}
      </Button>
      {error ? (
        <p className="app-lock-error" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
