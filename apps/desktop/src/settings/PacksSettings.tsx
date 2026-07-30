import { Button } from "@lattice/ui";
import { useEffect, useState } from "react";

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
import { getSemanticStatus, listenSemanticEvents, type SemanticStatus } from "../lib/semantic";
import { getVoiceStatus, listenVoiceEvents, type VoiceStatus } from "../lib/voice";
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

/** Catalog of downloadable first-party packs with status, download, and clear. */
export function PacksSettings({ workspaceRoot, onSemanticEnabledChange }: PacksSettingsProps) {
  const packs = listPacks();
  const [semantic, setSemantic] = useState<SemanticStatus | null>(null);
  const [voice, setVoice] = useState<VoiceStatus | null>(null);
  const [voiceError, setVoiceError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<PackId | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (inBrowser) return;
    let cancelled = false;
    void getVoiceStatus()
      .then((next) => {
        if (!cancelled) {
          setVoice(next);
          setVoiceError(null);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) setVoiceError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (inBrowser || !workspaceRoot) {
      setSemantic(null);
      return;
    }
    let cancelled = false;
    void getSemanticStatus(workspaceRoot)
      .then((next) => {
        if (!cancelled) setSemantic(next);
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
        setSemantic((prev) => ({
          state: event.state,
          pendingChunks: event.pendingChunks,
          message: event.message,
          progressPercent: event.progressPercent ?? null,
          providerId: event.providerId ?? prev?.providerId ?? null,
          modelId: event.modelId ?? prev?.modelId ?? null,
          dimensions: event.dimensions ?? prev?.dimensions ?? null,
        }));
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (inBrowser) return;
    let unlisten: (() => void) | undefined;
    void listenVoiceEvents((event) => {
      if (event.type === "status") {
        if (event.state === "preparing") {
          setVoiceError(null);
          setVoice((prev) =>
            prev
              ? { ...prev, preparing: true, message: event.message }
              : {
                  available: true,
                  prepared: false,
                  preparing: true,
                  listening: false,
                  nativeCapture: false,
                  platform: "macos",
                  message: event.message,
                },
          );
        }
        if (event.state === "ready") {
          setVoiceError(null);
          setVoice((prev) =>
            prev
              ? { ...prev, prepared: true, preparing: false, message: event.message }
              : {
                  available: true,
                  prepared: true,
                  preparing: false,
                  listening: false,
                  nativeCapture: false,
                  platform: "macos",
                  message: event.message,
                },
          );
        }
      }
      if (event.type === "failed") {
        setVoiceError(event.message);
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  // Poll while either pack is downloading so progress stays fresh.
  useEffect(() => {
    if (inBrowser) return;
    const semanticBusy =
      semantic?.state === "downloading" ||
      semantic?.state === "preparing" ||
      semantic?.state === "indexing";
    const voiceBusy = voice?.preparing === true;
    if (!semanticBusy && !voiceBusy) return;

    const id = window.setInterval(() => {
      if (workspaceRoot && semanticBusy) {
        void getSemanticStatus(workspaceRoot)
          .then(setSemantic)
          .catch(() => {
            /* keep last */
          });
      }
      if (voiceBusy) {
        void getVoiceStatus()
          .then((next) => {
            setVoice(next);
            setVoiceError(null);
          })
          .catch(() => {
            /* keep last */
          });
      }
    }, 750);
    return () => window.clearInterval(id);
  }, [workspaceRoot, semantic?.state, voice?.preparing]);

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
          setSemantic(result.status);
          onSemanticEnabledChange(true);
        } else {
          setVoice(result.status);
          setVoiceError(null);
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
        setSemantic(result.status);
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
          {!workspaceRoot ? (
            <div className="diagnostics-card" role="status">
              <strong>Open a workspace</strong>
              <span>The embedding pack needs an open workspace for status and download.</span>
            </div>
          ) : null}
          {error ? (
            <div className="diagnostics-card" role="alert">
              <strong>Pack error</strong>
              <span>{error}</span>
            </div>
          ) : null}
        </>
      )}
    </>
  );
}
