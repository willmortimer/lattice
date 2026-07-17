import type { Resource, ResourceKind } from "../types";

/** Shared drag MIME for Lattice sidebar/resource payloads. */
export const LATTICE_RESOURCE_MIME = "application/x-lattice-resource";

export type ResourceDropIntent = "link" | "embed" | "canvas-place";

export interface LatticeResourceDragPayload {
  version: 1;
  path: string;
  kind: ResourceKind;
  formatId?: string;
  title: string;
}

export function resourceDragTitle(resource: Pick<Resource, "path">): string {
  return resource.path.split("/").pop() ?? resource.path;
}

export function encodeResourceDragPayload(resource: Resource): string {
  const payload: LatticeResourceDragPayload = {
    version: 1,
    path: resource.path,
    kind: resource.kind,
    formatId: resource.formatId,
    title: resourceDragTitle(resource),
  };
  return JSON.stringify(payload);
}

export function decodeResourceDragPayload(raw: string | null | undefined): LatticeResourceDragPayload | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<LatticeResourceDragPayload>;
    if (parsed.version !== 1 || typeof parsed.path !== "string" || !parsed.path) return null;
    if (typeof parsed.kind !== "string") return null;
    return {
      version: 1,
      path: parsed.path,
      kind: parsed.kind as ResourceKind,
      formatId: typeof parsed.formatId === "string" ? parsed.formatId : undefined,
      title: typeof parsed.title === "string" && parsed.title ? parsed.title : parsed.path,
    };
  } catch {
    return null;
  }
}

export function readResourceDragPayload(data: DataTransfer | null | undefined): LatticeResourceDragPayload | null {
  if (!data) return null;
  return decodeResourceDragPayload(data.getData(LATTICE_RESOURCE_MIME));
}

export function writeResourceDragPayload(data: DataTransfer, resource: Resource): void {
  const encoded = encodeResourceDragPayload(resource);
  data.setData(LATTICE_RESOURCE_MIME, encoded);
  data.setData("text/plain", resource.path);
  data.effectAllowed = "copyMove";
}

/** Alt/Option prefers embed on pages; otherwise insert a wiki/path link. */
export function pageDropIntent(event: Pick<DragEvent, "altKey">): Exclude<ResourceDropIntent, "canvas-place"> {
  return event.altKey ? "embed" : "link";
}

export function wikiLinkMarkdown(payload: LatticeResourceDragPayload): string {
  const target = payload.title.replace(/\|/g, "\\|");
  return `[[${target}]]`;
}

export function latticeEmbedMarkdown(payload: LatticeResourceDragPayload): string {
  return `:::lattice-embed\nresource: ${payload.path}\nfallback: "[[${payload.title.replace(/"/g, '\\"')}]]"\n:::\n`;
}
