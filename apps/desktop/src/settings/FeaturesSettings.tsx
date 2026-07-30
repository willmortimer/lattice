import { Button } from "@lattice/ui";
import { useEffect, useState } from "react";

import { inBrowser } from "../demo";
import {
  downloadPack,
  getPack,
  semanticStatusToPackStatus,
  voiceStatusToPackStatus,
  type PackStatus,
} from "../lib/packs";
import { disableSemanticSearch, getSemanticStatus, listenSemanticEvents } from "../lib/semantic";
import { getVoiceStatus, listenVoiceEvents } from "../lib/voice";
import { packStatusLabel } from "./packStatusLabels";

export interface FeaturesSettingsProps {
  workspaceRoot: string | null;
  semanticEnabled: boolean;
  onSemanticEnabledChange: (semanticEnabled: boolean) => void;
  onOpenPacks: () => void;
  onOpenCapabilities: () => void;
}

function Toggle({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      className={`settings-toggle ${checked ? "settings-toggle-on" : ""}`}
      onClick={() => onChange(!checked)}
    >
      <span />
    </button>
  );
}

/** First-party feature toggles with pack dependency prompts. */
export function FeaturesSettings({
  workspaceRoot,
  semanticEnabled,
  onSemanticEnabledChange,
  onOpenPacks,
  onOpenCapabilities,
}: FeaturesSettingsProps) {
  const embeddingPack = getPack("embeddings.qwen3-0.6b");
  const voicePack = getPack("voice.parakeet-unified");
  const [embeddingStatus, setEmbeddingStatus] = useState<PackStatus>("missing");
  const [voiceStatus, setVoiceStatus] = useState<PackStatus>("missing");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (inBrowser || !workspaceRoot) {
      setEmbeddingStatus("missing");
      return;
    }
    let cancelled = false;
    void getSemanticStatus(workspaceRoot)
      .then((next) => {
        if (!cancelled) setEmbeddingStatus(semanticStatusToPackStatus(next));
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
    let cancelled = false;
    void getVoiceStatus()
      .then((next) => {
        if (!cancelled) setVoiceStatus(voiceStatusToPackStatus(next));
      })
      .catch((err: unknown) => {
        if (!cancelled) setVoiceStatus(voiceStatusToPackStatus(null, { error: String(err) }));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (inBrowser) return;
    let unlisten: (() => void) | undefined;
    void listenSemanticEvents((event) => {
      if (event.type === "status") {
        setEmbeddingStatus(
          semanticStatusToPackStatus({
            state: event.state,
            pendingChunks: event.pendingChunks,
            message: event.message,
            progressPercent: event.progressPercent ?? null,
          }),
        );
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
          setVoiceStatus("downloading");
        }
        if (event.state === "ready") {
          setVoiceStatus("ready");
        }
      }
      if (event.type === "failed") {
        setVoiceStatus(voiceStatusToPackStatus(null, { error: event.message }));
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  function handleSemanticToggle(next: boolean) {
    if (busy) return;
    if (!next) {
      onSemanticEnabledChange(false);
      if (!inBrowser && workspaceRoot) {
        setBusy(true);
        setError(null);
        void disableSemanticSearch(workspaceRoot)
          .then((status) => setEmbeddingStatus(semanticStatusToPackStatus(status)))
          .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)))
          .finally(() => setBusy(false));
      }
      return;
    }

    if (!workspaceRoot) {
      setError("Open a workspace before enabling semantic search.");
      return;
    }

    // Prompt with pack confirm copy when missing (or re-confirm enable); downloadPack
    // starts the worker even if the GGUF is already present.
    const accepted = window.confirm(embeddingPack.confirmCopy);
    if (!accepted) return;
    setBusy(true);
    setError(null);
    void downloadPack("embeddings.qwen3-0.6b", workspaceRoot)
      .then((result) => {
        if (result.kind === "semantic") {
          setEmbeddingStatus(semanticStatusToPackStatus(result.status));
        }
        onSemanticEnabledChange(true);
      })
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)))
      .finally(() => setBusy(false));
  }

  return (
    <>
      <h1>Features</h1>
      <p className="settings-copy">
        First-party capabilities that may need a Pack download. Shell surfaces such as canvas and
        terminal stay under Enabled capabilities.
      </p>
      {inBrowser ? (
        <div className="diagnostics-card">
          <strong>Unavailable in browser demo</strong>
          <span>Feature packs require the native desktop build.</span>
        </div>
      ) : (
        <>
          <div className="setting-row">
            <div>
              <strong>Semantic search</strong>
              <span>
                Include vector similarity alongside keyword matches. Needs{" "}
                {embeddingPack.title} ({embeddingPack.approxSizeLabel}). Pack status:{" "}
                {packStatusLabel(embeddingStatus)}.
              </span>
            </div>
            <div className="setting-control">
              <Toggle
                label="Semantic search"
                checked={semanticEnabled}
                disabled={busy}
                onChange={(checked) => void handleSemanticToggle(checked)}
              />
            </div>
          </div>

          <div className="setting-row">
            <div>
              <strong>Voice dictation</strong>
              <span>
                Hold-to-talk speech-to-text once {voicePack.title} is ready. Pack status:{" "}
                {packStatusLabel(voiceStatus)}. Enablement follows pack readiness — download under
                Packs.
              </span>
            </div>
            <div className="setting-control">
              <Button size="sm" variant="secondary" onClick={onOpenPacks}>
                Open Packs
              </Button>
            </div>
          </div>

          <div className="setting-row">
            <div>
              <strong>Agent memory</strong>
              <span>
                Remember / recall for the embedded agent uses the same embedding pack when
                available. No separate toggle yet — prepare embeddings under Packs.
              </span>
            </div>
            <div className="setting-control">
              <span className="settings-copy">
                {embeddingStatus === "ready" ? "Pack ready" : packStatusLabel(embeddingStatus)}
              </span>
            </div>
          </div>

          <h2 className="settings-subsection">Shell capabilities</h2>
          <p className="settings-copy">
            Canvas, SQLite, and terminal toggles live under Enabled capabilities.
          </p>
          <div className="setting-row">
            <div>
              <strong>Enabled capabilities</strong>
              <span>Show or hide bundled shell surfaces without changing Pack downloads.</span>
            </div>
            <div className="setting-control">
              <Button size="sm" variant="secondary" onClick={onOpenCapabilities}>
                Open Enabled capabilities
              </Button>
            </div>
          </div>

          {error ? (
            <div className="diagnostics-card" role="alert">
              <strong>Feature error</strong>
              <span>{error}</span>
            </div>
          ) : null}
        </>
      )}
    </>
  );
}
