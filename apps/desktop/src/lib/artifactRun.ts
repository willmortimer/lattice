import { invoke } from "@tauri-apps/api/core";

import type { BindingSpec } from "./bindingSpec";
import type { ArtifactBindingResultDto } from "./artifactBridge";

/** Validated artifact manifest DTO from `artifact_load_manifest`. */
export interface ArtifactManifestDto {
  format: string;
  version: number;
  title?: string | null;
  entrypoint: string;
  bindings: Record<string, BindingSpec>;
  permissions: {
    network: string[];
    workspaceWrite: string[];
  };
  fallback: {
    file?: string | null;
    text?: string | null;
  };
  packagePath: string;
}

export interface ArtifactEntrypointDto {
  html: string;
  entrypoint: string;
  packagePath: string;
  title?: string | null;
  bindingNames: string[];
}

/** Load and validate `artifact.yaml` for a workspace-relative `.artifact/` package. */
export function loadArtifactManifest(root: string, relPath: string): Promise<ArtifactManifestDto> {
  return invoke<ArtifactManifestDto>("artifact_load_manifest", {
    request: { root, relPath },
  });
}

/** Read the HTML entrypoint for sandboxed mounting. */
export function readArtifactEntrypoint(root: string, relPath: string): Promise<ArtifactEntrypointDto> {
  return invoke<ArtifactEntrypointDto>("artifact_read_entrypoint", {
    request: { root, relPath },
  });
}

/** Resolve a named read-only BindingSpec declared on the artifact. */
export function resolveArtifactBinding(
  root: string,
  relPath: string,
  name: string,
): Promise<ArtifactBindingResultDto> {
  return invoke<ArtifactBindingResultDto>("artifact_resolve_binding", {
    request: { root, relPath, name },
  });
}
