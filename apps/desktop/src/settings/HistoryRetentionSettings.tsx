import { Button } from "@lattice/ui";
import { useState } from "react";

import type { HistoryCleanupReport } from "../lib/revisions";
import {
  bytesToGibibytes,
  confirmHistoryCleanup,
  defaultHistoryRetentionControls,
  formatReclaimableBytes,
  gibibytesToBytes,
  previewHistoryCleanup,
  type HistoryRetentionControls,
} from "./historyRetention";

interface HistoryRetentionSettingsProps {
  workspaceRoot: string | null;
  nativeAvailable: boolean;
}

export function HistoryRetentionSettings({
  workspaceRoot,
  nativeAvailable,
}: HistoryRetentionSettingsProps) {
  const [controls, setControls] = useState<HistoryRetentionControls>(defaultHistoryRetentionControls);
  const [report, setReport] = useState<HistoryCleanupReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function runPreview() {
    if (!workspaceRoot || !nativeAvailable) return;
    setBusy(true);
    setError(null);
    try {
      setReport(await previewHistoryCleanup(workspaceRoot, controls));
    } catch (err) {
      setError(String(err));
      setReport(null);
    } finally {
      setBusy(false);
    }
  }

  async function runConfirm() {
    if (!workspaceRoot || !nativeAvailable) return;
    setBusy(true);
    setError(null);
    try {
      setReport(await confirmHistoryCleanup(workspaceRoot, controls));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="history-retention-settings">
      <div className="setting-row">
        <div>
          <strong>History payload age</strong>
          <span>Drop unpinned revision payloads older than this many days (metadata is kept).</span>
        </div>
        <div className="setting-control">
          <input
            type="number"
            min={1}
            max={3650}
            value={controls.maxAgeDays}
            aria-label="History retention age in days"
            onChange={(event) =>
              setControls((current) => ({
                ...current,
                maxAgeDays: Math.max(1, Math.min(3650, Number(event.currentTarget.value) || 1)),
              }))
            }
          />
        </div>
      </div>
      <div className="setting-row">
        <div>
          <strong>History payload budget</strong>
          <span>Bound retained payload objects per workspace (GiB). Pinned and baseline payloads are exempt.</span>
        </div>
        <div className="setting-control">
          <input
            type="number"
            min={0.1}
            max={100}
            step={0.1}
            value={bytesToGibibytes(controls.maxBytes)}
            aria-label="History retention budget in GiB"
            onChange={(event) =>
              setControls((current) => ({
                ...current,
                maxBytes: gibibytesToBytes(Math.max(0.1, Math.min(100, Number(event.currentTarget.value) || 0.1))),
              }))
            }
          />
        </div>
      </div>
      <div className="setting-row">
        <div>
          <strong>Cleanup</strong>
          <span>Preview reclaimable payloads before the first destructive cleanup.</span>
        </div>
        <div className="setting-control history-retention-actions">
          <Button type="button" variant="secondary" size="sm" disabled={!nativeAvailable || !workspaceRoot || busy} onClick={() => void runPreview()}>
            Preview cleanup
          </Button>
          {report?.requiresConfirmation ? (
            <Button type="button" variant="primary" size="sm" disabled={busy} onClick={() => void runConfirm()}>
              Confirm cleanup
            </Button>
          ) : null}
        </div>
      </div>
      {!nativeAvailable ? (
        <p className="history-retention-note" role="status">
          History cleanup requires the native desktop shell and an open workspace.
        </p>
      ) : null}
      {error ? (
        <p className="history-retention-error" role="alert">
          {error}
        </p>
      ) : null}
      {report ? (
        <div className="history-retention-report" role="status">
          {report.notice ? <p>{report.notice}</p> : null}
          <p>
            Reclaimable {formatReclaimableBytes(report.reclaimableBytes)} across {report.candidates.length}{" "}
            object{report.candidates.length === 1 ? "" : "s"}
            {report.deletedObjects > 0
              ? ` · deleted ${report.deletedObjects} (${formatReclaimableBytes(report.deletedBytes)})`
              : ""}
            .
          </p>
        </div>
      ) : null}
    </div>
  );
}
