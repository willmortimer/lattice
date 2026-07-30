import { useMemo } from "react";

import { useAgentSessionStore } from "./agentStore";
import {
  defaultModelForProvider,
  modelsForProvider,
  type SelectableAgentProvider,
} from "./modelCatalog";
import { agentProviderLabel, resolveAgentProviderKind } from "./providerKind";

function isSelectableProvider(value: string | null): value is SelectableAgentProvider {
  return value === "openai" || value === "pioneer";
}

export function AgentProviderBadge() {
  const healthBackend = useAgentSessionStore((state) => state.healthBackend);
  const healthModel = useAgentSessionStore((state) => state.healthModel);
  const healthOk = useAgentSessionStore((state) => state.healthOk);
  const healthDegraded = useAgentSessionStore((state) => state.healthDegraded);
  const lastEventBackend = useAgentSessionStore((state) => state.lastEventBackend);
  const selectedProvider = useAgentSessionStore((state) => state.selectedProvider);
  const selectedModel = useAgentSessionStore((state) => state.selectedModel);
  const aiMode = useAgentSessionStore((state) => state.aiMode);
  const accountAiDisabled = useAgentSessionStore((state) => state.accountAiDisabled);
  const setSelectedProvider = useAgentSessionStore((state) => state.setSelectedProvider);
  const setSelectedModel = useAgentSessionStore((state) => state.setSelectedModel);

  const kind = useMemo(
    () => resolveAgentProviderKind(healthBackend, lastEventBackend),
    [healthBackend, lastEventBackend],
  );

  if (accountAiDisabled) {
    return (
      <div className="agent-runtime-controls" aria-label="Agent runtime">
        <span className="agent-provider-badge agent-provider-badge-unknown">
          Lattice Account · Coming soon
        </span>
        <span className="agent-runtime-hint">
          Cloud AI is not available yet. Switch to Local or BYO OpenAI in Settings → AI.
        </span>
      </div>
    );
  }

  const live = kind !== "fake" && kind !== "unknown";
  const providerValue: SelectableAgentProvider =
    selectedProvider ?? (isSelectableProvider(kind) ? kind : "openai");
  const providerOptions: SelectableAgentProvider[] =
    aiMode === "byoOpenai" ? ["openai"] : ["openai", "pioneer"];
  const modelOptions = modelsForProvider(providerValue);
  const modelValue =
    selectedModel ?? healthModel ?? defaultModelForProvider(providerValue);

  const statusLabel =
    kind === "fake"
      ? "Fake"
      : healthDegraded
        ? "Degraded"
        : healthOk === false
          ? "Down"
          : "Live";

  const offlineHint =
    aiMode === "byoOpenai"
      ? "Add your OpenAI API key in Settings → AI"
      : "Launch with secrets/ai.env via nxr desktop-dev";

  return (
    <div className="agent-runtime-controls" aria-label="Agent runtime">
      <span className={`agent-provider-badge agent-provider-badge-${kind}`}>
        {agentProviderLabel(kind)} · {statusLabel}
      </span>
      {live ? (
        <>
          <label className="agent-runtime-field">
            <span className="agent-runtime-field-label">Provider</span>
            <select
              className="agent-runtime-select"
              value={providerValue}
              aria-label="Agent provider"
              onChange={(event) =>
                setSelectedProvider(event.target.value as SelectableAgentProvider)
              }
            >
              {providerOptions.map((option) => (
                <option key={option} value={option}>
                  {option === "openai" ? "OpenAI" : "Pioneer"}
                </option>
              ))}
            </select>
          </label>
          <label className="agent-runtime-field">
            <span className="agent-runtime-field-label">Model</span>
            <select
              className="agent-runtime-select"
              value={modelValue}
              aria-label="Agent model"
              onChange={(event) => setSelectedModel(event.target.value)}
            >
              {modelOptions.map((option) => (
                <option key={option.id} value={option.id}>
                  {option.label}
                </option>
              ))}
              {!modelOptions.some((option) => option.id === modelValue) && (
                <option value={modelValue}>{modelValue}</option>
              )}
            </select>
          </label>
        </>
      ) : (
        <span className="agent-runtime-hint" title={offlineHint}>
          No live keys — fake backend
        </span>
      )}
    </div>
  );
}
