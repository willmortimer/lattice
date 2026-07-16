import { invoke } from "@tauri-apps/api/core";

import type { Resource, ResourceKind } from "../types";

export interface ResourceLinkTarget {
  canonical: string;
  display: string;
  path: string;
  kind: ResourceKind;
}

export type ResourceLinkResolution =
  | { status: "found"; target: ResourceLinkTarget; anchor: string | null }
  | {
      status: "ambiguous";
      query: string;
      candidates: ResourceLinkTarget[];
      anchor: string | null;
    }
  | { status: "missing"; query: string; suggested_page: string | null; anchor: string | null };

export function demoLinkTargets(resources: Resource[]): ResourceLinkTarget[] {
  return resources.map((resource) => ({
    path: resource.path,
    kind: resource.kind,
    display: resource.path.split("/").pop() ?? resource.path,
    canonical:
      resource.kind === "page"
        ? resource.path.replace(/\.(md|markdown)$/i, "")
        : resource.kind === "folder"
          ? `${resource.path}/`
          : resource.path,
  }));
}

export async function searchResourceLinks(
  root: string,
  query: string,
  limit = 20,
): Promise<ResourceLinkTarget[]> {
  return invoke("search_resource_links", { root, query, limit });
}

export async function resolveResourceLink(
  root: string,
  sourcePath: string | null,
  target: string,
): Promise<ResourceLinkResolution> {
  return invoke("resolve_resource_link", { root, sourcePath, target });
}

export async function refreshResourceCatalog(root: string): Promise<void> {
  await invoke("refresh_resource_catalog", { root });
}
