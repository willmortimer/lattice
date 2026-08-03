import { Button } from "@lattice/ui";
import { useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";

import { inBrowser } from "../demo";
import { cloudBlobOpen } from "../lib/cloud";
import {
  backupResourceToCloud,
  openCloudAccountSettings,
} from "../lib/cloudBackup";
import {
  downloadPack,
  getPack,
  semanticStatusToPackStatus,
  voiceStatusToPackStatus,
} from "../lib/packs";
import { formatAuthority, type ResourceStat } from "../lib/resourceStat";
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
  const [labsPath, setLabsPath] = useState("Notes.md");
  const [labsBusy, setLabsBusy] = useState(false);
  const [labsError, setLabsError] = useState<string | null>(null);
  const [labsStat, setLabsStat] = useState<ResourceStat | null>(null);
  const [labsOpenBytes, setLabsOpenBytes] = useState<number | null>(null);

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

  async function handleLabsUpload() {
    if (inBrowser || labsBusy) return;
    if (!workspaceRoot) {
      setLabsError("Open a workspace before uploading to cloud.");
      return;
    }
    const relPath = labsPath.trim();
    if (!relPath) {
      setLabsError("Enter a workspace-relative path to upload.");
      return;
    }
    setLabsBusy(true);
    setLabsError(null);
    setLabsOpenBytes(null);
    try {
      const result = await backupResourceToCloud(workspaceRoot, relPath);
      if (!result.ok) {
        if (result.reason === "signed_out") {
          openCloudAccountSettings();
        }
        setLabsStat(null);
        setLabsError(result.message);
        return;
      }
      setLabsStat(result.stat);
    } finally {
      setLabsBusy(false);
    }
  }

  async function handleLabsReopen() {
    if (inBrowser || labsBusy) return;
    if (!workspaceRoot) {
      setLabsError("Open a workspace before reopening from cloud.");
      return;
    }
    const relPath = labsPath.trim();
    if (!relPath) {
      setLabsError("Enter a workspace-relative path to reopen.");
      return;
    }
    setLabsBusy(true);
    setLabsError(null);
    try {
      const bytes = await cloudBlobOpen(workspaceRoot, relPath);
      setLabsOpenBytes(bytes.length);
    } catch (err: unknown) {
      setLabsOpenBytes(null);
      setLabsError(err instanceof Error ? err.message : String(err));
    } finally {
      setLabsBusy(false);
    }
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

          <h2 className="settings-subsection">Labs</h2>
          <p className="settings-copy">
            Advanced cloud blob round-trip for any workspace-relative path. For day-to-day backup,
            use Inspect, the Files tree context menu, or the command palette on a selected resource.
            Requires Settings → Cloud account sign-in.
          </p>
          <SettingRow
            settingId="features.labs-cloud-blob"
            title="Labs cloud blob put/get"
            description="Experimental path-based upload and reopen — prefer tree or Inspect for normal backup."
          >
            <div className="cloud-signin-password">
              <label className="cloud-signin-field">
                <span>Workspace-relative path</span>
                <input
                  type="text"
                  value={labsPath}
                  disabled={labsBusy}
                  placeholder="Notes.md"
                  onChange={(event) => setLabsPath(event.currentTarget.value)}
                />
              </label>
              <div className="cloud-account-actions">
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={labsBusy}
                  onClick={() => void handleLabsUpload()}
                >
                  {labsBusy ? "Working…" : "Labs: upload path to cloud"}
                </Button>
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={labsBusy}
                  onClick={() => void handleLabsReopen()}
                >
                  {labsBusy ? "Working…" : "Reopen from cloud"}
                </Button>
              </div>
            </div>
          </SettingRow>
          {labsStat ? (
            <div className="diagnostics-card" role="status">
              <strong>Cloud materialize ok</strong>
              <span>
                {labsStat.path} · authority {formatAuthority(labsStat.authority)}
                {labsStat.content_hash ? ` · ${labsStat.content_hash}` : ""}
              </span>
            </div>
          ) : null}
          {labsOpenBytes !== null ? (
            <div className="diagnostics-card" role="status">
              <strong>Reopened from cloud</strong>
              <span>{labsOpenBytes} bytes</span>
            </div>
          ) : null}
          {labsError ? (
            <div className="diagnostics-card" role="alert">
              <strong>Labs cloud blob error</strong>
              <span>{labsError}</span>
            </div>
          ) : null}

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
