/**
 * Narrow postMessage bridge between the host shell and a sandboxed artifact iframe.
 * Artifacts never receive ambient Tauri; only these typed messages cross the boundary.
 */

import type { BindingSpec } from "./bindingSpec";

export const ARTIFACT_BRIDGE_PREFIX = "lattice.artifact." as const;

export type ArtifactHostToFrameMessage =
  | {
      type: "lattice.artifact.init";
      title?: string | null;
      bindings: string[];
    }
  | {
      type: "lattice.artifact.theme";
      vars: Record<string, string>;
      background?: string;
      appearance?: string;
    }
  | {
      type: "lattice.artifact.bindingResult";
      id: string;
      ok: boolean;
      data?: ArtifactBindingResultDto;
      error?: string;
    };

export type ArtifactFrameToHostMessage =
  | {
      type: "lattice.artifact.requestBinding";
      id: string;
      name: string;
    }
  | {
      type: "lattice.artifact.openResource";
      path: string;
    }
  | {
      type: "lattice.artifact.notify";
      title?: string;
      height?: number;
    };

export type ArtifactBindingResultDto =
  | {
      kind: "scalar";
      column?: string | null;
      value?: unknown;
      binding: BindingSpec;
    }
  | {
      kind: "resource";
      path: string;
      binding: BindingSpec;
    }
  | {
      kind: "saved-view";
      resource: string;
      view: string;
      binding: BindingSpec;
    }
  | {
      kind: "unsupported";
      message: string;
      binding: BindingSpec;
    };

export function isArtifactFrameMessage(value: unknown): value is ArtifactFrameToHostMessage {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  if (typeof record.type !== "string" || !record.type.startsWith(ARTIFACT_BRIDGE_PREFIX)) {
    return false;
  }
  switch (record.type) {
    case "lattice.artifact.requestBinding":
      return typeof record.id === "string" && typeof record.name === "string";
    case "lattice.artifact.openResource":
      return typeof record.path === "string";
    case "lattice.artifact.notify":
      return (
        (record.title === undefined || typeof record.title === "string") &&
        (record.height === undefined || typeof record.height === "number")
      );
    default:
      return false;
  }
}

export function isArtifactHostMessage(value: unknown): value is ArtifactHostToFrameMessage {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  if (typeof record.type !== "string" || !record.type.startsWith(ARTIFACT_BRIDGE_PREFIX)) {
    return false;
  }
  switch (record.type) {
    case "lattice.artifact.init":
      return Array.isArray(record.bindings) && record.bindings.every((item) => typeof item === "string");
    case "lattice.artifact.theme":
      return (
        !!record.vars &&
        typeof record.vars === "object" &&
        !Array.isArray(record.vars) &&
        Object.values(record.vars as Record<string, unknown>).every((item) => typeof item === "string")
      );
    case "lattice.artifact.bindingResult":
      return typeof record.id === "string" && typeof record.ok === "boolean";
    default:
      return false;
  }
}
