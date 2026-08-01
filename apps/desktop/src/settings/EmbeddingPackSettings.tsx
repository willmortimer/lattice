import { Button } from "@lattice/ui";
import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { inBrowser } from "../demo";
import type { EmbeddingMode } from "../lib/profile";
import {
  enableSemanticSearch,
  isSemanticPackPrepared,
  isVectorsBehindStatus,
  SEMANTIC_MODEL_CONFIRM,
  semanticProviderLabel,
  semanticStatusLabel,
  VECTORS_BEHIND_EXPLANATION,
} from "../lib/semantic";
import {
  setSemanticStatusCache,
  useSemanticStatusQuery,
} from "../query/useSemanticStatusQuery";

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

function queryErrorMessage(error: unknown): string | null {
  if (!error) return null;
  return error instanceof Error ? error.message : String(error);
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
  const queryClient = useQueryClient();
  const { data: status = null, error: statusQueryError } =
    useSemanticStatusQuery(workspaceRoot);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const displayError = error ?? queryErrorMessage(statusQueryError);

  function handleDownloadPack() {
    if (!workspaceRoot || busy || isSemanticPackPrepared(status)) return;
    const accepted = window.confirm(SEMANTIC_MODEL_CONFIRM);
    if (!accepted) return;
    setBusy(true);
    setError(null);
    void enableSemanticSearch(workspaceRoot)
      .then((next) => {
        setSemanticStatusCache(queryClient, workspaceRoot, next);
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
        setSemanticStatusCache(queryClient, workspaceRoot, next);
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
        {passiveEmbeddingEnabled ? "on" : "off"} (change under Embedding defaults above). Pack
        catalog and feature toggles are managed under Packs / Features.
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
          {displayError ? (
            <div className="diagnostics-card" role="alert">
              <strong>Embedding pack error</strong>
              <span>{displayError}</span>
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
