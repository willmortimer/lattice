import { invoke as tauriInvoke } from "@tauri-apps/api/core";

/** How the React shell reaches the Rust command core. */
export type IpcMode = "tauri" | "bridge" | "demo";

const STALE_REVISION_PREFIX = "STALE_REVISION:";

/** Handler routes exposed by `lattice-bridge` (see `apps/bridge/README.md`). */
export const BRIDGE_COMMANDS = new Set([
  "open_workspace",
  "list_resources",
  "read_page",
  "apply_page_update",
  "create_page",
  "search_workspace",
  "rebuild_index",
  "ensure_home",
  "list_templates",
  "create_workspace",
  "get_backlinks",
]);

export function resolveIpcMode(
  hasTauriInternals: boolean,
  bridgeUrlValue: string | undefined,
): IpcMode {
  if (hasTauriInternals) return "tauri";
  if (bridgeUrlValue?.trim()) return "bridge";
  return "demo";
}

export const hasTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/**
 * Bridge base URL (`VITE_LATTICE_BRIDGE_URL`). In containerized Vite dev this
 * points at `lattice-bridge` (default `http://127.0.0.1:8787`).
 */
export const bridgeUrl =
  (import.meta.env.VITE_LATTICE_BRIDGE_URL as string | undefined)?.trim() || undefined;

/**
 * Optional workspace root for bridge mode (`VITE_LATTICE_WORKSPACE`). Bridge has
 * no native folder dialog — set this env var for auto-open on load, or use Get
 * started (`ensure_home` seeds First Look under `LATTICE_DEV_HOME` on the
 * bridge host). The bridge may also be started with `--root` so `root` can be
 * omitted on workspace-scoped routes.
 */
export const bridgeWorkspacePath =
  (import.meta.env.VITE_LATTICE_WORKSPACE as string | undefined)?.trim() || undefined;

export const ipcMode = resolveIpcMode(hasTauri, bridgeUrl);
export const inBridgeMode = ipcMode === "bridge";

export function isBridgeCommand(command: string): boolean {
  return BRIDGE_COMMANDS.has(command);
}

export class UnsupportedIpcCommandError extends Error {
  constructor(command: string, mode: IpcMode) {
    super(
      mode === "bridge"
        ? `${command} is not available in bridge mode (Tauri-only).`
        : `${command} is not available in browser demo mode.`,
    );
    this.name = "UnsupportedIpcCommandError";
  }
}

function normalizeBridgeResponse<T>(command: string, payload: unknown): T {
  if (
    (command === "apply_page_update" || command === "create_page") &&
    typeof payload === "object" &&
    payload !== null &&
    "revision" in payload
  ) {
    return (payload as { revision: string }).revision as T;
  }
  return payload as T;
}

function bridgeErrorMessage(payload: unknown, fallback: string): string {
  if (typeof payload === "object" && payload !== null && "error" in payload) {
    return String((payload as { error: string }).error);
  }
  return fallback;
}

/** POST JSON to a bridge handler route; shared by `invoke` and tests. */
export async function postBridgeCommand<T>(
  baseUrl: string,
  command: string,
  args: Record<string, unknown> | undefined,
  fetchImpl: typeof fetch = fetch,
): Promise<T> {
  if (!isBridgeCommand(command)) {
    throw new UnsupportedIpcCommandError(command, "bridge");
  }

  const url = `${baseUrl.replace(/\/$/, "")}/${command}`;
  const response = await fetchImpl(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args ?? {}),
  });

  const text = await response.text();
  let payload: unknown = null;
  if (text) {
    try {
      payload = JSON.parse(text) as unknown;
    } catch {
      payload = text;
    }
  }

  if (!response.ok) {
    const message = bridgeErrorMessage(payload, text || response.statusText);
    if (response.status === 409 || message.startsWith(STALE_REVISION_PREFIX)) {
      throw message.startsWith(STALE_REVISION_PREFIX)
        ? message
        : `${STALE_REVISION_PREFIX}${message}`;
    }
    throw new Error(message);
  }

  return normalizeBridgeResponse<T>(command, payload);
}

/**
 * Typed command transport: Tauri IPC, lattice-bridge HTTP, or demo-only error.
 * MVP commands are routed through all three; everything else is Tauri-only.
 */
export async function invoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  switch (ipcMode) {
    case "tauri":
      return tauriInvoke<T>(command, args);
    case "bridge":
      if (!bridgeUrl) {
        throw new Error("VITE_LATTICE_BRIDGE_URL is not configured.");
      }
      return postBridgeCommand<T>(bridgeUrl, command, args);
    case "demo":
      throw new UnsupportedIpcCommandError(command, "demo");
    default: {
      const unreachable: never = ipcMode;
      throw new Error(`Unknown IPC mode: ${unreachable}`);
    }
  }
}
