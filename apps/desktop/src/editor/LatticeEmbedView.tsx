import { NodeViewWrapper } from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";
import {
  Cube,
  Database,
  FileImage,
  FilePdf,
  FileText,
  Package,
  Plugs,
  TerminalWindow,
} from "@phosphor-icons/react";
import { useEffect, useRef, useState, type CSSProperties } from "react";

import { ArtifactSandbox } from "../artifacts/ArtifactSandbox";
import { loadArtifactManifest, type ArtifactManifestDto } from "../lib/artifactRun";
import { useAssetContext } from "./AssetContext";
import { parseEmbedMode } from "./directives";
import {
  loadEmbedPreview,
  type EmbedPreviewKind,
  type EmbedPreviewResult,
} from "./embedPreview";

const KIND_LABEL: Record<EmbedPreviewKind, string> = {
  page: "Page",
  image: "Image",
  pdf: "PDF",
  "data-app": "Data",
  artifact: "Artifact",
  interface: "Interface",
  task: "Task",
  unknown: "Resource",
};

function kindIcon(kind: EmbedPreviewKind) {
  switch (kind) {
    case "page":
      return FileText;
    case "image":
      return FileImage;
    case "pdf":
      return FilePdf;
    case "data-app":
      return Database;
    case "artifact":
      return Package;
    case "interface":
      return Plugs;
    case "task":
      return TerminalWindow;
    default:
      return Cube;
  }
}

function parseEmbedHeight(height: string | null | undefined): CSSProperties | undefined {
  const trimmed = height?.trim();
  if (!trimmed) return undefined;
  if (/^\d+$/.test(trimmed)) return { height: `${trimmed}px` };
  return { height: trimmed };
}

/**
 * Read-view card / interactive surface for `:::lattice-embed` blocks.
 * `mode: interactive` mounts the shared artifact sandbox when the target is
 * an artifact; other kinds open via the host or show a lightweight preview.
 */
export function LatticeEmbedView({ node }: NodeViewProps) {
  const { root, pagePath, onOpenEmbed } = useAssetContext();
  const resource = (node.attrs.resource as string) || "";
  const view = (node.attrs.view as string | null) ?? undefined;
  const mode = parseEmbedMode((node.attrs.mode as string | null) ?? undefined);
  const height = (node.attrs.height as string | null) ?? undefined;
  const lines = (node.attrs.lines as string | null) ?? undefined;
  const missing = resource.trim().length === 0;

  const [preview, setPreview] = useState<EmbedPreviewResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [artifactManifest, setArtifactManifest] = useState<ArtifactManifestDto | null>(null);
  const cardRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (missing) {
      setPreview(null);
      setLoading(false);
      return;
    }

    const controller = new AbortController();
    let imageRevoke: (() => void) | undefined;
    let started = false;
    let observer: IntersectionObserver | null = null;

    const beginLoad = () => {
      if (started || controller.signal.aborted) return;
      started = true;
      setLoading(true);
      void loadEmbedPreview(
        { resource, view, height, lines, mode },
        { root, pagePath },
        controller.signal,
      )
        .then((result) => {
          if (controller.signal.aborted) return;
          imageRevoke = result?.imageRevoke;
          setPreview(result);
        })
        .catch((error: unknown) => {
          if (controller.signal.aborted) return;
          setPreview({
            kind: "unknown",
            resolvedPath: resource,
            label: resource,
            unavailable: error instanceof Error ? error.message : String(error),
          });
        })
        .finally(() => {
          if (!controller.signal.aborted) setLoading(false);
        });
    };

    const element = cardRef.current;
    if (!element || typeof IntersectionObserver === "undefined") {
      beginLoad();
    } else {
      observer = new IntersectionObserver(
        (entries) => {
          if (!entries.some((entry) => entry.isIntersecting)) return;
          beginLoad();
          observer?.disconnect();
        },
        { rootMargin: "120px" },
      );
      observer.observe(element);
    }

    return () => {
      observer?.disconnect();
      controller.abort();
      imageRevoke?.();
    };
  }, [height, lines, missing, mode, pagePath, resource, root, view]);

  useEffect(() => {
    if (mode !== "interactive" || preview?.kind !== "artifact" || !root || !preview.resolvedPath) {
      setArtifactManifest(null);
      return;
    }
    let cancelled = false;
    void loadArtifactManifest(root, preview.resolvedPath)
      .then((manifest) => {
        if (!cancelled) setArtifactManifest(manifest);
      })
      .catch(() => {
        if (!cancelled) setArtifactManifest(null);
      });
    return () => {
      cancelled = true;
    };
  }, [mode, preview?.kind, preview?.resolvedPath, root]);

  const previewKind = preview?.kind ?? "unknown";
  const KindIcon = kindIcon(previewKind);
  const heightStyle = parseEmbedHeight(height);
  const showInteractiveArtifact =
    mode === "interactive" && previewKind === "artifact" && artifactManifest && root && preview?.resolvedPath;
  const interactive = Boolean(onOpenEmbed && preview?.resolvedPath) && !showInteractiveArtifact;

  const handleActivate = () => {
    if (!onOpenEmbed || !preview?.resolvedPath) return;
    onOpenEmbed(preview.resolvedPath);
  };

  return (
    <NodeViewWrapper className="page-embed-lattice">
      {showInteractiveArtifact ? (
        <div
          ref={cardRef}
          className="page-embed-lattice-interactive artifact-embed"
          style={heightStyle}
          role="figure"
          aria-label={`Interactive artifact embed: ${resource}`}
        >
          <div className="page-embed-lattice-header">
            <KindIcon size={14} aria-hidden="true" />
            <span className="page-embed-lattice-label">Artifact embed · interactive</span>
            {preview?.label ? <span className="page-embed-lattice-title">{preview.label}</span> : null}
          </div>
          <ArtifactSandbox
            root={root}
            packagePath={preview.resolvedPath}
            manifest={artifactManifest}
            height={height}
            onOpenResource={onOpenEmbed}
          />
        </div>
      ) : (
        <div
          ref={cardRef}
          className={`page-embed-lattice-card${missing ? " page-embed-lattice-card--degraded" : ""}${interactive ? " page-embed-lattice-card--interactive" : ""}`}
          style={heightStyle}
          role="figure"
          aria-label={missing ? "Embed missing resource" : `Embed: ${resource}`}
          onClick={interactive ? handleActivate : undefined}
          onKeyDown={
            interactive
              ? (event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    handleActivate();
                  }
                }
              : undefined
          }
          tabIndex={interactive ? 0 : undefined}
        >
          <div className="page-embed-lattice-header">
            <KindIcon size={14} aria-hidden="true" />
            <span className="page-embed-lattice-label">
              {missing
                ? "Embed"
                : `${KIND_LABEL[previewKind]} embed${mode !== "card" ? ` · ${mode}` : ""}`}
            </span>
            {preview?.label && !missing && (
              <span className="page-embed-lattice-title">{preview.label}</span>
            )}
          </div>

          {missing ? (
            <p className="page-embed-lattice-degraded">Missing required resource path.</p>
          ) : (
            <div className="page-embed-lattice-body">
              {loading && !preview ? (
                <p className="page-embed-lattice-status" role="status">
                  Loading preview…
                </p>
              ) : null}

              {preview?.excerpt ? (
                <pre className="page-embed-lattice-excerpt">{preview.excerpt}</pre>
              ) : null}

              {preview?.imageUrl ? (
                <div className="page-embed-lattice-image-wrap">
                  <img
                    className="page-embed-lattice-image"
                    src={preview.imageUrl}
                    alt={preview.label}
                    loading="lazy"
                    decoding="async"
                  />
                </div>
              ) : null}

              {preview?.kind === "pdf" ? (
                <div className="page-embed-lattice-chip" aria-label={`PDF: ${preview.label}`}>
                  <FilePdf size={16} aria-hidden="true" />
                  <span>PDF · open resource</span>
                </div>
              ) : null}

              {preview?.kind === "data-app" ? (
                <dl className="page-embed-lattice-data">
                  {preview.dataTitle ? (
                    <>
                      <dt>Package</dt>
                      <dd>{preview.dataTitle}</dd>
                    </>
                  ) : null}
                  {preview.dataView ? (
                    <>
                      <dt>View</dt>
                      <dd>{preview.dataView}</dd>
                    </>
                  ) : null}
                  {preview.dataTable ? (
                    <>
                      <dt>Table</dt>
                      <dd>{preview.dataTable}</dd>
                    </>
                  ) : null}
                </dl>
              ) : null}

              {preview?.kind === "artifact" || preview?.kind === "interface" || preview?.kind === "task" ? (
                <p className="page-embed-lattice-resource">
                  {mode === "interactive"
                    ? "Open resource for the live surface (interactive embed mounts artifacts inline)."
                    : preview.resolvedPath}
                </p>
              ) : null}

              {preview?.unavailable ? (
                <p className="page-embed-lattice-status">{preview.unavailable}</p>
              ) : null}

              {!loading &&
              preview &&
              !preview.excerpt &&
              !preview.imageUrl &&
              preview.kind !== "pdf" &&
              preview.kind !== "data-app" &&
              preview.kind !== "artifact" &&
              preview.kind !== "interface" &&
              preview.kind !== "task" ? (
                <p className="page-embed-lattice-resource">{preview.resolvedPath}</p>
              ) : null}
            </div>
          )}

          {!missing && preview?.resolvedPath ? (
            <p className="page-embed-lattice-path">{preview.resolvedPath}</p>
          ) : null}
        </div>
      )}
    </NodeViewWrapper>
  );
}
