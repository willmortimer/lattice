import { Button } from "@lattice/ui";
import { useEffect, useState } from "react";

import { inBrowser } from "../demo";
import type { EmbeddingMode } from "../lib/profile";
import {
  enableSemanticSearch,
  getSemanticStatus,
  isSemanticPackPrepared,
  isVectorsBehindStatus,
  listenSemanticEvents,
  SEMANTIC_MODEL_CONFIRM,
  semanticProviderLabel,
  semanticStatusLabel,
  VECTORS_BEHIND_EXPLANATION,
  type SemanticStatus,
} from "../lib/semantic";

function embeddingModeLabel(mode: EmbeddingMode): string {
  switch (mode) {
    case "followAi":
      return "Follow AI mode";
    case "local":
      return "Local";
    case "remote":
      return "Remote";
    default: {
      const _exhaustive: never = mode;
      return _exhaustive;
    }
  }
}

export interface EmbeddingPackSettingsProps {
  workspaceRoot: string | null;
  semanticEnabled: boolean;
  onSemanticEnabledChange: (semanticEnabled: boolean) => void;
  embeddingMode: EmbeddingMode;
  passiveEmbeddingEnabled: boolean;
}

/** Optional local embedding pack download, status, and Lance freshness under Settings → AI. */
export function EmbeddingPackSettings({
  workspaceRoot,
  semanticEnabled,
  onSemanticEnabledChange,
  embeddingMode,
  passiveEmbeddingEnabled,
}: EmbeddingPackSettingsProps) {
  const [status, setStatus] = useState<SemanticStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (inBrowser || !workspaceRoot) return;
    let cancelled = false;
    void getSemanticStatus(workspaceRoot)
      .then((next) => {
        if (!cancelled) setStatus(next);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceRoot]);

  useEffect(() => {
    if (inBrowser) return;
    let unlisten: (() => void) | undefined;
    void listenSemanticEvents((event) => {
      if (event.type === "status") {
        setStatus((prev) => {
          const nextPercent = event.progressPercent ?? null;
          const progressPercent =
            event.state === "downloading" &&
            prev?.state === "downloading" &&
            prev.progressPercent != null &&
            nextPercent != null
              ? Math.max(prev.progressPercent, nextPercent)
              : nextPercent;
          return {
            state: event.state,
            pendingChunks: event.pendingChunks,
            message: event.message,
            progressPercent,
            providerId: event.providerId ?? prev?.providerId ?? null,
            modelId: event.modelId ?? prev?.modelId ?? null,
            dimensions: event.dimensions ?? prev?.dimensions ?? null,
          };
        });
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  // Poll while downloading / preparing / indexing so progress and stale detection stay fresh.
  useEffect(() => {
    if (inBrowser || !workspaceRoot || !semanticEnabled) return;
    if (
      !status ||
      (status.state !== "downloading" &&
        status.state !== "preparing" &&
        status.state !== "indexing")
    ) {
      return;
    }
    const id = window.setInterval(() => {
      void getSemanticStatus(workspaceRoot)
        .then((next) => {
          setStatus((prev) => {
            if (
              next.state === "downloading" &&
              prev?.state === "downloading" &&
              prev.progressPercent != null &&
              next.progressPercent != null
            ) {
              return {
                ...next,
                progressPercent: Math.max(prev.progressPercent, next.progressPercent),
              };
            }
            return next;
          });
        })
        .catch(() => {
          /* keep last status */
        });
    }, 750);
    return () => window.clearInterval(id);
  }, [workspaceRoot, semanticEnabled, status?.state]);

  function handleDownloadPack() {
    if (!workspaceRoot || busy || isSemanticPackPrepared(status)) return;
    const accepted = window.confirm(SEMANTIC_MODEL_CONFIRM);
    if (!accepted) return;
    setBusy(true);
    setError(null);
    void enableSemanticSearch(workspaceRoot)
      .then((next) => {
        setStatus(next);
        onSemanticEnabledChange(true);
      })
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)))
      .finally(() => setBusy(false));
  }

  function handleRefreshVectors() {
    if (!workspaceRoot || busy) return;
    setBusy(true);
    setError(null);
    void enableSemanticSearch(workspaceRoot)
      .then((next) => {
        setStatus(next);
        if (!semanticEnabled) onSemanticEnabledChange(true);
      })
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)))
      .finally(() => setBusy(false));
  }

  const packPrepared = isSemanticPackPrepared(status);
  const vectorsBehind = status != null && isVectorsBehindStatus(status);
  const statusText = status
    ? semanticStatusLabel(
        status.state,
        status.pendingChunks,
        status.progressPercent,
        status.message,
      )
    : semanticEnabled
      ? "Preparing…"
      : "Not prepared";
  const providerText = status ? semanticProviderLabel(status) : null;
  const downloadLabel = busy && !packPrepared ? "Downloading…" : packPrepared ? "Downloaded" : "Download pack";

  return (
    <div data-slot="embedding-pack-settings">
      <h2 className="settings-subsection">Embeddings</h2>
      <p className="settings-copy">
        Optional local embedding pack for semantic search. Mode is{" "}
        {embeddingModeLabel(embeddingMode).toLowerCase()}; passive embedding is{" "}
        {passiveEmbeddingEnabled ? "on" : "off"} (change under Embedding defaults above). Search
        still owns the semantic search toggle.
      </p>
      {inBrowser ? (
        <div className="diagnostics-card">
          <strong>Unavailable in browser demo</strong>
          <span>Embeddings require the native desktop build with latticed indexing services.</span>
        </div>
      ) : !workspaceRoot ? (
        <div className="diagnostics-card" role="status">
          <strong>Open a workspace</strong>
          <span>Embedding pack status and download need an open workspace.</span>
        </div>
      ) : (
        <>
          <div className="setting-row">
            <div>
              <strong>Embedding pack</strong>
              <span>
                Download Qwen3-Embedding-0.6B (Q8, ~640 MB, Apache-2.0). The model stays on this Mac.
              </span>
            </div>
            <div className="setting-control">
              <Button
                size="sm"
                disabled={busy || packPrepared}
                onClick={() => void handleDownloadPack()}
              >
                {downloadLabel}
              </Button>
            </div>
          </div>
          <div className="setting-row">
            <div>
              <strong>Pack status</strong>
              <span>Whether the local embedding model and Lance index are ready for this workspace.</span>
            </div>
            <div className="setting-control">
              <span>
                {busy && !packPrepared ? (
                  "Updating…"
                ) : (
                  <>
                    {statusText}
                    {providerText ? (
                      <>
                        <br />
                        <span className="settings-copy">Provider: {providerText}</span>
                      </>
                    ) : null}
                  </>
                )}
              </span>
            </div>
          </div>
          {vectorsBehind ? (
            <div className="diagnostics-card" role="status">
              <strong>Vectors behind workspace</strong>
              <span>{VECTORS_BEHIND_EXPLANATION}</span>
              <div className="ai-account-actions">
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={busy}
                  onClick={() => void handleRefreshVectors()}
                >
                  {busy ? "Refreshing…" : "Refresh vectors"}
                </Button>
              </div>
            </div>
          ) : null}
          {error ? (
            <div className="diagnostics-card" role="alert">
              <strong>Embedding pack error</strong>
              <span>{error}</span>
            </div>
          ) : null}
          {status?.message && status.state === "failed" ? (
            <div className="diagnostics-card" role="status">
              <strong>Details</strong>
              <span>{status.message}</span>
            </div>
          ) : null}
        </>
      )}
    </div>
  );
}
