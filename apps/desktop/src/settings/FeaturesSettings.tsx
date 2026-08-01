import { Button } from "@lattice/ui";
import { useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";

import { inBrowser } from "../demo";
import {
  downloadPack,
  getPack,
  semanticStatusToPackStatus,
  voiceStatusToPackStatus,
} from "../lib/packs";
import { disableSemanticSearch } from "../lib/semantic";
import {
  setSemanticStatusCache,
  useSemanticStatusQuery,
} from "../query/useSemanticStatusQuery";
import { useVoiceStatusQuery } from "../query/useVoiceStatusQuery";
import { packStatusLabel } from "./packStatusLabels";
import { SettingRow } from "./SettingRow";

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

function queryErrorMessage(error: unknown): string | null {
  if (!error) return null;
  return error instanceof Error ? error.message : String(error);
}

/** First-party feature toggles with pack dependency prompts. */
export function FeaturesSettings({
  workspaceRoot,
  semanticEnabled,
  onSemanticEnabledChange,
  onOpenPacks,
  onOpenCapabilities,
}: FeaturesSettingsProps) {
  const queryClient = useQueryClient();
  const embeddingPack = getPack("embeddings.qwen3-0.6b");
  const voicePack = getPack("voice.parakeet-unified");
  const { data: semanticStatus = null, error: semanticQueryError } =
    useSemanticStatusQuery(workspaceRoot);
  const { data: voice = null, error: voiceQueryError } = useVoiceStatusQuery();
  const embeddingStatus = useMemo(
    () => semanticStatusToPackStatus(semanticStatus),
    [semanticStatus],
  );
  const voiceStatus = useMemo(
    () =>
      voiceStatusToPackStatus(voice, {
        error: queryErrorMessage(voiceQueryError),
      }),
    [voice, voiceQueryError],
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const displayError = error ?? queryErrorMessage(semanticQueryError);

  function handleSemanticToggle(next: boolean) {
    if (busy) return;
    if (!next) {
      onSemanticEnabledChange(false);
      if (!inBrowser && workspaceRoot) {
        setBusy(true);
        setError(null);
        void disableSemanticSearch(workspaceRoot)
          .then((status) => setSemanticStatusCache(queryClient, workspaceRoot, status))
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
          setSemanticStatusCache(queryClient, workspaceRoot, result.status);
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
          <SettingRow
            settingId="features.semantic"
            title="Semantic search"
            description={`Include vector similarity alongside keyword matches. Needs ${embeddingPack.title} (${embeddingPack.approxSizeLabel}). Pack status: ${packStatusLabel(embeddingStatus)}.`}
          >
            <Toggle
              label="Semantic search"
              checked={semanticEnabled}
              disabled={busy}
              onChange={(checked) => void handleSemanticToggle(checked)}
            />
          </SettingRow>

          <SettingRow
            settingId="features.voice"
            title="Voice dictation"
            description={`Hold-to-talk speech-to-text once ${voicePack.title} is ready. Pack status: ${packStatusLabel(voiceStatus)}. Enablement follows pack readiness — download under Packs.`}
          >
            <Button size="sm" variant="secondary" onClick={onOpenPacks}>
              Open Packs
            </Button>
          </SettingRow>

          <SettingRow
            settingId="features.memory"
            title="Agent memory"
            description="Remember / recall for the embedded agent uses the same embedding pack when available. No separate toggle yet — prepare embeddings under Packs."
          >
            <span className="settings-copy">
              {embeddingStatus === "ready" ? "Pack ready" : packStatusLabel(embeddingStatus)}
            </span>
          </SettingRow>

          <h2 className="settings-subsection">Shell capabilities</h2>
          <p className="settings-copy">
            Canvas, SQLite, and terminal toggles live under Enabled capabilities.
          </p>
          <SettingRow
            settingId="capabilities.canvas"
            title="Enabled capabilities"
            description="Show or hide bundled shell surfaces without changing Pack downloads."
          >
              <Button size="sm" variant="secondary" onClick={onOpenCapabilities}>
                Open Enabled capabilities
              </Button>
          </SettingRow>

          {displayError ? (
            <div className="diagnostics-card" role="alert">
              <strong>Feature error</strong>
              <span>{displayError}</span>
            </div>
          ) : null}
        </>
      )}
    </>
  );
}
