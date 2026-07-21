import { invoke } from "@tauri-apps/api/core";

import { demoTextFiles } from "../demoWorkspace.generated";

/** Virtual workspace root inside the Pyodide filesystem. */
export const PYODIDE_WORKSPACE_ROOT = "/home/pyodide/workspace";

/** Default First Look paths copied into Pyodide before a cell run. */
export const DEFAULT_BRIDGED_WORKSPACE_PATHS = [
  "Data/Orders.dataset/sources/orders.csv",
] as const;

/** Soft cap for bridged file bytes (matches desktop editor asset budget). */
export const MAX_BRIDGED_FILE_BYTES = 8 * 1024 * 1024;

export type BridgedWorkspaceFile = {
  /** Workspace-relative POSIX path (e.g. `Data/Orders.dataset/sources/orders.csv`). */
  path: string;
  /** Absolute path under {@link PYODIDE_WORKSPACE_ROOT}. */
  mountPath: string;
  bytes: Uint8Array;
  /** Where the bytes came from — disk vs sticky-workspace template fallback. */
  source: "workspace" | "demo-template";
};

export type WorkspaceBridgeUnavailableReason =
  | "browser-demo"
  | "no-root"
  | "read-failed";

export type WorkspaceBridgeResult =
  | { ok: true; files: BridgedWorkspaceFile[]; notice?: string }
  | {
      ok: false;
      reason: WorkspaceBridgeUnavailableReason;
      message: string;
    };

/** Normalize a workspace-relative path (no leading slash, no `..` segments). */
export function normalizeWorkspaceRelPath(relPath: string): string {
  const trimmed = relPath.trim().replace(/\\/g, "/").replace(/^\/+/, "");
  const parts = trimmed.split("/").filter((part) => part.length > 0 && part !== ".");
  if (parts.some((part) => part === "..")) {
    throw new Error(`Workspace path must not contain '..': ${relPath}`);
  }
  if (parts.length === 0) {
    throw new Error("Workspace path must not be empty.");
  }
  return parts.join("/");
}

/** Map a workspace-relative path to its Pyodide mount location. */
export function pyodideMountPath(relPath: string): string {
  return `${PYODIDE_WORKSPACE_ROOT}/${normalizeWorkspaceRelPath(relPath)}`;
}

/**
 * Infer Pyodide `loadPackage` names from cell source.
 * Keeps first Run of stub notebooks light when they do not import analytics libs.
 */
export function packagesForNotebookCode(code: string): string[] {
  const packages: string[] = [];
  if (/\b(?:import\s+pandas|from\s+pandas\b|\bpd\.read_csv\b)/.test(code)) {
    packages.push("pandas");
  }
  if (/\b(?:import\s+matplotlib|from\s+matplotlib\b|\bplt\.)/.test(code)) {
    packages.push("matplotlib");
  }
  return packages;
}

function toUint8Array(bytes: ArrayBuffer | number[] | Uint8Array): Uint8Array {
  if (bytes instanceof Uint8Array) return bytes;
  if (bytes instanceof ArrayBuffer) return new Uint8Array(bytes);
  return new Uint8Array(bytes);
}

async function readWorkspaceBytes(root: string, relPath: string): Promise<Uint8Array> {
  const response = await invoke<ArrayBuffer | number[] | Uint8Array>("read_binary_file", {
    root,
    relPath,
  });
  return toUint8Array(response);
}

function demoTemplateBytes(relPath: string): Uint8Array | null {
  const text = demoTextFiles[relPath];
  if (typeof text !== "string") return null;
  return new TextEncoder().encode(text);
}

/**
 * Read a bridged path from the open workspace, falling back to the compiled
 * First Look template when a sticky on-disk folder is missing seed files.
 */
async function readBridgedBytes(
  root: string,
  path: string,
): Promise<{ bytes: Uint8Array; source: "workspace" | "demo-template" }> {
  try {
    return { bytes: await readWorkspaceBytes(root, path), source: "workspace" };
  } catch (error) {
    const fallback = demoTemplateBytes(path);
    if (fallback) {
      return { bytes: fallback, source: "demo-template" };
    }
    throw error;
  }
}

/**
 * Read selected workspace files for a one-shot copy into the Pyodide FS.
 * Browser demo / missing root return an honest unavailable result (no silent fake mount).
 */
export async function prepareWorkspaceBridge(options: {
  root: string | null;
  inBrowser: boolean;
  paths?: readonly string[];
}): Promise<WorkspaceBridgeResult> {
  if (options.inBrowser) {
    return {
      ok: false,
      reason: "browser-demo",
      message:
        "Workspace CSV bridge needs the native desktop app with an open workspace. The browser fixture cannot mount workspace files into Pyodide.",
    };
  }
  const root = options.root?.trim() ?? "";
  if (!root) {
    return {
      ok: false,
      reason: "no-root",
      message:
        "Workspace CSV bridge unavailable — open a workspace folder in the native app to mount Orders CSV into Pyodide.",
    };
  }

  const paths = options.paths ?? DEFAULT_BRIDGED_WORKSPACE_PATHS;
  const files: BridgedWorkspaceFile[] = [];
  try {
    for (const rawPath of paths) {
      const path = normalizeWorkspaceRelPath(rawPath);
      const { bytes, source } = await readBridgedBytes(root, path);
      if (bytes.byteLength > MAX_BRIDGED_FILE_BYTES) {
        return {
          ok: false,
          reason: "read-failed",
          message: `Bridged file exceeds ${MAX_BRIDGED_FILE_BYTES} bytes: ${path}`,
        };
      }
      files.push({ path, mountPath: pyodideMountPath(path), bytes, source });
    }
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    return {
      ok: false,
      reason: "read-failed",
      message: `${detail} — create a fresh First Look workspace (or copy missing seeds from templates/workspaces/demo/files/). Installing Lattice.app does not rewrite an existing First Look folder.`,
    };
  }

  const templatePaths = files.filter((file) => file.source === "demo-template").map((file) => file.path);
  const notice =
    templatePaths.length > 0
      ? `Mounted First Look template copy for missing disk file(s): ${templatePaths.join(", ")}. Create a fresh First Look workspace to persist these seeds on disk.`
      : undefined;

  return { ok: true, files, notice };
}
