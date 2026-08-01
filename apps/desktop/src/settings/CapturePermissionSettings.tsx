import { Button } from "@lattice/ui";
import { useEffect, useState } from "react";

import { inBrowser } from "../demo";
import {
  capturePermissionLabel,
  getCapturePermissionStatus,
  openCapturePermissionSettings,
  requestCapturePermission,
  type CapturePermissionStatus,
} from "../lib/capturePermission";

function SettingRow({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <div className="setting-row">
      <div>
        <strong>{title}</strong>
        <span>{description}</span>
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

/** macOS Screen Recording permission status and setup affordances. */
export function CapturePermissionSettings() {
  const [status, setStatus] = useState<CapturePermissionStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (inBrowser) return;
    let cancelled = false;
    void getCapturePermissionStatus()
      .then((next) => {
        if (!cancelled) setStatus(next);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const statusText = capturePermissionLabel(status, { busy, error });
  const canRequest =
    status?.available === true &&
    status.state === "notDetermined" &&
    !busy &&
    !error;
  const canOpenSettings =
    status?.available === true &&
    (status.state === "denied" || status.state === "restricted") &&
    !busy;

  function handleRequest() {
    if (!canRequest) return;
    setBusy(true);
    setError(null);
    void requestCapturePermission()
      .then((next) => setStatus(next))
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)))
      .finally(() => setBusy(false));
  }

  function handleOpenSettings() {
    if (!canOpenSettings) return;
    setBusy(true);
    setError(null);
    void openCapturePermissionSettings()
      .then(() => getCapturePermissionStatus())
      .then((next) => setStatus(next))
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)))
      .finally(() => setBusy(false));
  }

  return (
    <>
      <h2>Screen capture</h2>
      <p className="settings-section-lead">
        Screen clips use macOS Screen Recording permission. Captures stay on this Mac and save to
        your workspace Capture Inbox.
      </p>
      {inBrowser ? (
        <p className="settings-hint">Screen capture permission requires the native desktop app.</p>
      ) : (
        <>
          <SettingRow
            title="Screen recording"
            description={status?.reason ?? "Required for screen clips and Capture Inbox ingest."}
          >
            <span className="settings-status-pill">{statusText}</span>
          </SettingRow>
          {status?.state === "notDetermined" ? (
            <div className="settings-actions">
              <Button size="sm" onClick={handleRequest} disabled={!canRequest}>
                Allow screen recording
              </Button>
            </div>
          ) : null}
          {canOpenSettings ? (
            <div className="settings-actions">
              <Button size="sm" variant="secondary" onClick={handleOpenSettings} disabled={busy}>
                Open System Settings
              </Button>
            </div>
          ) : null}
          {status?.message ? (
            <p className="settings-hint">{status.message}</p>
          ) : null}
          {error ? (
            <div className="settings-error" role="alert">
              <strong>Screen capture permission error</strong>
              <span>{error}</span>
            </div>
          ) : null}
        </>
      )}
    </>
  );
}
