import { invoke } from "../lib/ipc";

import type { ResourceInspection } from "../lib/resourceRuntime";
import { inspectResource } from "../lib/resourceRuntime";
import { loadImageAsset, type ImageAsset } from "../viewers/media/imageSource";
import { resolveWorkspaceAssetPath } from "./assets";
import { splitFrontmatter } from "./markdown";

export const DEFAULT_EMBED_EXCERPT_LINES = 8;

export type EmbedPreviewKind =
  | "page"
  | "image"
  | "pdf"
  | "data-app"
  | "artifact"
  | "interface"
  | "task"
  | "unknown";

export interface EmbedPreviewContext {
  root: string | null;
  pagePath: string;
}

export interface EmbedPreviewAttrs {
  resource: string;
  view?: string | null;
  height?: string | null;
  lines?: string | null;
  mode?: string | null;
}

export interface DataAppEmbedTarget {
  packagePath: string;
  viewName: string | null;
}

export interface EmbedPreviewResult {
  kind: EmbedPreviewKind;
  resolvedPath: string;
  label: string;
  excerpt?: string;
  imageUrl?: string;
  imageRevoke?: () => void;
  dataTitle?: string;
  dataView?: string;
  dataTable?: string;
  unavailable?: string;
}

export interface EmbedPreviewRuntime {
  inspect: (location: { root: string; path: string }, signal: AbortSignal) => Promise<ResourceInspection>;
  readPage: (root: string, path: string, signal: AbortSignal) => Promise<string>;
  loadImage: (location: { root: string; path: string }, signal: AbortSignal) => Promise<ImageAsset>;
  loadDataView: (
    root: string,
    packagePath: string,
    viewName: string,
    signal: AbortSignal,
  ) => Promise<{ name: string; table: string }>;
}

const IMAGE_FORMAT_IDS = new Set(["image", "file:image"]);
const PDF_FORMAT_IDS = new Set(["pdf", "file:pdf"]);

function defaultRuntime(): EmbedPreviewRuntime {
  return {
    inspect: (location, signal) => inspectResource(location, signal),
    readPage: async (root, path, signal) => {
      const page = await invoke<{ content: string }>("read_page", { root, relPath: path });
      if (signal.aborted) throw new DOMException("Embed preview was cancelled", "AbortError");
      return page.content;
    },
    loadImage: (location, signal) => loadImageAsset(location, signal),
    loadDataView: async (root, packagePath, viewName, signal) => {
      const summary = await invoke<{ name: string; table: string }>("load_data_view", {
        root,
        relPath: packagePath,
        name: viewName,
      });
      if (signal.aborted) throw new DOMException("Embed preview was cancelled", "AbortError");
      return summary;
    },
  };
}

/** Parse `lines` as a count (`8`) or inclusive range (`10-20`). */
export function parseEmbedLinesAttr(lines: string | null | undefined): { start: number; count: number } {
  const trimmed = lines?.trim();
  if (!trimmed) return { start: 0, count: DEFAULT_EMBED_EXCERPT_LINES };
  const range = /^(\d+)\s*-\s*(\d+)$/.exec(trimmed);
  if (range) {
    const start = Math.max(0, Number.parseInt(range[1]!, 10) - 1);
    const end = Math.max(start, Number.parseInt(range[2]!, 10) - 1);
    return { start, count: end - start + 1 };
  }
  const count = Number.parseInt(trimmed, 10);
  if (Number.isFinite(count) && count > 0) return { start: 0, count };
  return { start: 0, count: DEFAULT_EMBED_EXCERPT_LINES };
}

/** Build a bounded markdown excerpt from a page's raw on-disk content. */
export function excerptPageBody(raw: string, linesAttr?: string | null): string {
  const { body } = splitFrontmatter(raw);
  const { start, count } = parseEmbedLinesAttr(linesAttr);
  const lines = body.replace(/\r\n/g, "\n").split("\n");
  const slice = lines.slice(start, start + count);
  const excerpt = slice.join("\n").trimEnd();
  if (start + count < lines.length && excerpt.length > 0) {
    return `${excerpt}\n…`;
  }
  return excerpt;
}

/** Split a workspace-relative embed target into a `.data` package and optional view. */
export function parseDataAppEmbedPath(path: string): DataAppEmbedTarget {
  const normalized = path.replace(/\\/g, "/");
  const viewMatch = /^(.+\.data)\/views\/([^/]+)\.view\.yaml$/i.exec(normalized);
  if (viewMatch) {
    return { packagePath: viewMatch[1]!, viewName: viewMatch[2]! };
  }
  const packageMatch = /^(.+\.data)(?:\/.*)?$/i.exec(normalized);
  if (packageMatch) {
    return { packagePath: packageMatch[1]!, viewName: null };
  }
  return { packagePath: normalized, viewName: null };
}

export function resourceLabel(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  const base = normalized.includes("/") ? normalized.slice(normalized.lastIndexOf("/") + 1) : normalized;
  return base || normalized;
}

export function inferEmbedKind(
  inspection: ResourceInspection | null,
  resolvedPath: string,
): EmbedPreviewKind {
  const normalized = resolvedPath.replace(/\\/g, "/").toLowerCase();
  if (normalized.includes(".artifact")) return "artifact";
  if (normalized.includes(".task")) return "task";
  if (normalized.includes(".interface.yaml") || /\/interfaces\//.test(normalized)) return "interface";
  if (normalized.includes(".data")) return "data-app";

  if (inspection) {
    if (inspection.kind === "page" || inspection.profile === "markdown") return "page";
    if (inspection.kind === "data-app" || inspection.profile === "sqlite-data-app") return "data-app";
    if (IMAGE_FORMAT_IDS.has(inspection.profile)) return "image";
    if (PDF_FORMAT_IDS.has(inspection.profile)) return "pdf";
  }

  if (normalized.endsWith(".md") || normalized.endsWith(".markdown")) return "page";
  if (/\.(png|jpe?g|gif|webp|avif|bmp|tiff|svg)$/i.test(normalized)) return "image";
  if (normalized.endsWith(".pdf")) return "pdf";
  return "unknown";
}

function resolveEmbedPath(context: EmbedPreviewContext, resource: string): string | null {
  if (!resource.trim()) return null;
  if (!context.root) return resource.replace(/\\/g, "/");
  return resolveWorkspaceAssetPath(context.pagePath, resource);
}

export async function loadEmbedPreview(
  attrs: EmbedPreviewAttrs,
  context: EmbedPreviewContext,
  signal: AbortSignal,
  runtime: EmbedPreviewRuntime = defaultRuntime(),
): Promise<EmbedPreviewResult | null> {
  const resolvedPath = resolveEmbedPath(context, attrs.resource);
  if (!resolvedPath) return null;

  const label = resourceLabel(resolvedPath);

  if (!context.root) {
    const kind = inferEmbedKind(null, resolvedPath);
    return {
      kind,
      resolvedPath,
      label,
      unavailable: "Preview needs a native workspace.",
      ...(kind === "data-app" ? dataAppStub(resolvedPath, attrs.view) : {}),
    };
  }

  let inspection: ResourceInspection | null = null;
  try {
    inspection = await runtime.inspect({ root: context.root, path: resolvedPath }, signal);
  } catch {
    inspection = null;
  }

  const kind = inferEmbedKind(inspection, resolvedPath);

  switch (kind) {
    case "page": {
      try {
        const raw = await runtime.readPage(context.root, resolvedPath, signal);
        return { kind, resolvedPath, label, excerpt: excerptPageBody(raw, attrs.lines) };
      } catch (error) {
        return {
          kind,
          resolvedPath,
          label,
          unavailable: error instanceof Error ? error.message : String(error),
        };
      }
    }
    case "image": {
      try {
        const asset = await runtime.loadImage({ root: context.root, path: resolvedPath }, signal);
        return {
          kind,
          resolvedPath,
          label,
          imageUrl: asset.lease.url,
          imageRevoke: asset.lease.revoke,
        };
      } catch (error) {
        return {
          kind,
          resolvedPath,
          label,
          unavailable: error instanceof Error ? error.message : String(error),
        };
      }
    }
    case "pdf":
      return { kind, resolvedPath, label };
    case "data-app":
      return loadDataAppPreview(attrs, context.root, resolvedPath, signal, runtime);
    case "artifact":
    case "interface":
    case "task":
      return { kind, resolvedPath, label };
    default:
      return { kind, resolvedPath, label };
  }
}

function dataAppStub(resolvedPath: string, viewAttr?: string | null): Pick<EmbedPreviewResult, "dataTitle" | "dataView" | "dataTable"> {
  const target = parseDataAppEmbedPath(resolvedPath);
  const packageLabel = resourceLabel(target.packagePath).replace(/\.data$/i, "");
  const viewName = viewAttr?.trim() || target.viewName;
  return {
    dataTitle: packageLabel,
    dataView: viewName ?? undefined,
    dataTable: undefined,
  };
}

async function loadDataAppPreview(
  attrs: EmbedPreviewAttrs,
  root: string,
  resolvedPath: string,
  signal: AbortSignal,
  runtime: EmbedPreviewRuntime,
): Promise<EmbedPreviewResult> {
  const target = parseDataAppEmbedPath(resolvedPath);
  const viewName = attrs.view?.trim() || target.viewName;
  const stub = dataAppStub(resolvedPath, attrs.view);

  if (!viewName) {
    return {
      kind: "data-app",
      resolvedPath,
      label: resourceLabel(resolvedPath),
      ...stub,
    };
  }

  try {
    const summary = await runtime.loadDataView(root, target.packagePath, viewName, signal);
    return {
      kind: "data-app",
      resolvedPath,
      label: resourceLabel(resolvedPath),
      dataTitle: stub.dataTitle,
      dataView: summary.name,
      dataTable: summary.table,
    };
  } catch (error) {
    return {
      kind: "data-app",
      resolvedPath,
      label: resourceLabel(resolvedPath),
      ...stub,
      unavailable: error instanceof Error ? error.message : String(error),
    };
  }
}
