import { useEffect, useState } from "react";

import { ConflictEnvelope } from "../editor/ConflictEnvelope";
import type { ResourceRevisionDetail, ResourceRevisionSummary } from "../lib/revisions";
import {
  formatRevisionDiff,
  guardedRevertResourceRevision,
  loadResourceHistory,
  loadResourceHistoryDetail,
  resolveExpectedCurrentRevision,
} from "./inspectorHistoryActions";

function revisionLabel(item: ResourceRevisionSummary): string {
  return item.summary?.trim() || item.revisionId.slice(0, 12);
}

export function InspectorHistoryPanel({
  root,
  path,
  currentRevision = null,
}: {
  root: string;
  path: string;
  /** Session revision when the open resource matches `path`; otherwise list baseline. */
  currentRevision?: string | null;
}) {
  const [revisions, setRevisions] = useState<ResourceRevisionSummary[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<ResourceRevisionDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [detailLoading, setDetailLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reverting, setReverting] = useState(false);
  const [deferNote, setDeferNote] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setSelectedId(null);
    setDetail(null);
    setDeferNote(null);
    void loadResourceHistory(root, path)
      .then((items) => {
        if (cancelled) return;
        setRevisions(items);
        if (items[0]) setSelectedId(items[0].revisionId);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setRevisions([]);
        setError(err instanceof Error ? err.message : "Failed to load revision history.");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [root, path]);

  useEffect(() => {
    if (!selectedId) {
      setDetail(null);
      return;
    }
    let cancelled = false;
    setDetailLoading(true);
    setError(null);
    void loadResourceHistoryDetail(root, path, selectedId)
      .then((next) => {
        if (!cancelled) setDetail(next);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setDetail(null);
        setError(err instanceof Error ? err.message : "Failed to load revision detail.");
      })
      .finally(() => {
        if (!cancelled) setDetailLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [root, path, selectedId]);

  const expectedCurrent = resolveExpectedCurrentRevision(revisions, currentRevision);
  const canRevert = Boolean(selectedId && expectedCurrent && selectedId !== expectedCurrent && !reverting);

  async function onRevert() {
    if (!selectedId || !expectedCurrent) return;
    setReverting(true);
    setError(null);
    try {
      await guardedRevertResourceRevision({
        root,
        path,
        revisionId: selectedId,
        expectedCurrentRevision: expectedCurrent,
      });
      const items = await loadResourceHistory(root, path);
      setRevisions(items);
      const nextId = items[0]?.revisionId ?? selectedId;
      setSelectedId(nextId);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Revert failed.");
    } finally {
      setReverting(false);
    }
  }

  if (loading) return <p className="inspector-empty">Loading revisions…</p>;
  if (error && revisions.length === 0) return <p className="inspector-empty">{error}</p>;
  if (revisions.length === 0) return <p className="inspector-empty">No revisions for this resource yet.</p>;

  return (
    <div className="inspector-history-panel">
      {error && <p className="inspector-empty" role="alert">{error}</p>}
      {deferNote && <p className="inspector-empty">{deferNote}</p>}
      {detail?.conflict && (
        <ConflictEnvelope
          message={detail.conflict.failureReason || "Unresolved revision conflict."}
          actions={[
            {
              label: "Defer",
              onClick: () => setDeferNote("Conflict resolution deferred — reopen Inspect later."),
            },
          ]}
        />
      )}
      <div className="history-list" role="list" aria-label="Resource revisions">
        {revisions.map((item) => (
          <button
            type="button"
            key={item.revisionId}
            className={
              item.revisionId === selectedId
                ? "inspector-history-item inspector-history-item-active"
                : "inspector-history-item"
            }
            role="listitem"
            onClick={() => setSelectedId(item.revisionId)}
          >
            <strong>{revisionLabel(item)}</strong>
            <span>
              {new Date(item.createdAt * 1000).toLocaleString()}
              {item.currentBaseline ? " · current" : ""}
              {item.source === "external" ? " · external" : ""}
              {item.unresolvedConflict ? " · conflict" : ""}
              {!item.priorAvailable ? " · prior unavailable" : ""}
            </span>
          </button>
        ))}
      </div>
      <div className="inspector-history-detail">
        {detailLoading && <p className="inspector-empty">Loading detail…</p>}
        {!detailLoading && detail && (
          <>
            <pre className="inspector-source">{formatRevisionDiff(detail)}</pre>
            <button
              type="button"
              className="secondary-button"
              disabled={!canRevert}
              title={
                expectedCurrent
                  ? undefined
                  : "Revert unavailable until the current revision is known."
              }
              onClick={() => void onRevert()}
            >
              {reverting ? "Reverting…" : "Revert to this revision"}
            </button>
          </>
        )}
      </div>
    </div>
  );
}
