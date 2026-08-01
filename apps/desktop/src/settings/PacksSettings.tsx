import { Button } from "@lattice/ui";
import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { inBrowser } from "../demo";
import {
  clearPack,
  downloadPack,
  isPackClearSupported,
  listPacks,
  semanticStatusToPackStatus,
  voiceStatusToPackStatus,
  type PackId,
  type PackStatus,
} from "../lib/packs";
import type { SemanticStatus } from "../lib/semantic";
import type { VoiceStatus } from "../lib/voice";
import {
  setSemanticStatusCache,
  useSemanticStatusQuery,
} from "../query/useSemanticStatusQuery";
import { setVoiceStatusCache, useVoiceStatusQuery } from "../query/useVoiceStatusQuery";
import {
  isPackDownloadDisabled,
  packDownloadButtonLabel,
  packStatusLabel,
} from "./packStatusLabels";

export interface PacksSettingsProps {
  workspaceRoot: string | null;
  /** When the embedding pack is cleared, turn off semantic search preference. */
  onSemanticEnabledChange: (semanticEnabled: boolean) => void;
}

function packStatusForId(
  id: PackId,
  semantic: SemanticStatus | null,
  voice: VoiceStatus | null,
  voiceError: string | null,
): PackStatus {
  switch (id) {
    case "embeddings.qwen3-0.6b":
      return semanticStatusToPackStatus(semantic);
    case "voice.parakeet-unified":
      return voiceStatusToPackStatus(voice, { error: voiceError });
    default: {
      const _exhaustive: never = id;
      return _exhaustive;
    }
  }
}

function queryErrorMessage(error: unknown): string | null {
  if (!error) return null;
  return error instanceof Error ? error.message : String(error);
}

/** Catalog of downloadable first-party packs with status, download, and clear. */
export function PacksSettings({ workspaceRoot, onSemanticEnabledChange }: PacksSettingsProps) {
  const queryClient = useQueryClient();
  const packs = listPacks();
  const { data: semantic = null, error: semanticQueryError } =
    useSemanticStatusQuery(workspaceRoot);
  const { data: voice = null, error: voiceQueryError } = useVoiceStatusQuery();
  const voiceError = queryErrorMessage(voiceQueryError);
  const [busyId, setBusyId] = useState<PackId | null>(null);
  const [error, setError] = useState<string | null>(null);

  const displayError =
    error ??
    queryErrorMessage(semanticQueryError);

  function handleDownload(id: PackId) {
    const pack = packs.find((item) => item.id === id);
    if (!pack || busyId) return;
    if (id === "embeddings.qwen3-0.6b" && !workspaceRoot) {
      setError("Open a workspace before downloading the embedding pack.");
      return;
    }
    const status = packStatusForId(id, semantic, voice, voiceError);
    if (isPackDownloadDisabled(status, false)) return;
    const accepted = window.confirm(pack.confirmCopy);
    if (!accepted) return;
    setBusyId(id);
    setError(null);
    void downloadPack(id, workspaceRoot ?? "")
      .then((result) => {
        if (result.kind === "semantic") {
          if (workspaceRoot) {
            setSemanticStatusCache(queryClient, workspaceRoot, result.status);
          }
          onSemanticEnabledChange(true);
        } else {
          setVoiceStatusCache(queryClient, result.status);
        }
      })
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)))
      .finally(() => setBusyId(null));
  }

  function handleClear(id: PackId) {
    if (!isPackClearSupported(id) || busyId) return;
    if (!workspaceRoot) {
      setError("Open a workspace before clearing the embedding pack.");
      return;
    }
    const accepted = window.confirm(
      "Clear the embedding pack for this workspace? Semantic search will turn off until you download again.",
    );
    if (!accepted) return;
    setBusyId(id);
    setError(null);
    void clearPack(id, workspaceRoot)
      .then((result) => {
        setSemanticStatusCache(queryClient, workspaceRoot, result.status);
        onSemanticEnabledChange(false);
      })
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)))
      .finally(() => setBusyId(null));
  }

  return (
    <>
      <h1>Packs</h1>
      <p className="settings-copy">
        Downloadable artifacts that Features depend on. Models stay on this Mac; downloads need a
        network once.
      </p>
      {inBrowser ? (
        <div className="diagnostics-card">
          <strong>Unavailable in browser demo</strong>
          <span>Pack downloads require the native desktop build.</span>
        </div>
      ) : (
        <>
          <div data-setting-id="packs.catalog">
          {packs.map((pack) => {
            const status = packStatusForId(pack.id, semantic, voice, voiceError);
            const busy = busyId === pack.id;
            const clearSupported = isPackClearSupported(pack.id);
            const needsWorkspace =
              pack.id === "embeddings.qwen3-0.6b" && !workspaceRoot;
            return (
              <div key={pack.id} className="setting-row">
                <div>
                  <strong>{pack.title}</strong>
                  <span>
                    {pack.description} {pack.approxSizeLabel}, {pack.license}. Status:{" "}
                    {packStatusLabel(status)}.
                  </span>
                </div>
                <div className="setting-control">
                  <div className="history-retention-actions">
                    <Button
                      size="sm"
                      disabled={
                        needsWorkspace || isPackDownloadDisabled(status, busy)
                      }
                      onClick={() => void handleDownload(pack.id)}
                    >
                      {packDownloadButtonLabel(status, busy)}
                    </Button>
                    <Button
                      size="sm"
                      variant="secondary"
                      disabled={
                        !clearSupported ||
                        needsWorkspace ||
                        busy ||
                        status === "missing"
                      }
                      title={
                        clearSupported ? undefined : "Clear is not available yet"
                      }
                      onClick={() => void handleClear(pack.id)}
                    >
                      Clear
                    </Button>
                  </div>
                </div>
              </div>
            );
          })}
          </div>
          {!workspaceRoot ? (
            <div className="diagnostics-card" role="status">
              <strong>Open a workspace</strong>
              <span>The embedding pack needs an open workspace for status and download.</span>
            </div>
          ) : null}
          {displayError ? (
            <div className="diagnostics-card" role="alert">
              <strong>Pack error</strong>
              <span>{displayError}</span>
              {displayError.includes("auth token") || displayError.includes("LATTICE_AUTH_TOKEN") ? (
                <span>
                  Quit other Lattice / latticed processes, or remove{" "}
                  <code>~/Library/Application Support/Lattice/run/latticed.sock</code> and reopen
                  Packs. A fresh desktop launch writes <code>latticed.token</code> beside the socket.
                </span>
              ) : null}
            </div>
          ) : null}
        </>
      )}
    </>
  );
}
