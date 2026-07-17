import { NodeViewWrapper } from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";
import {
  Cube,
  Database,
  FileImage,
  FilePdf,
  FileText,
} from "@phosphor-icons/react";
import { useEffect, useRef, useState, type CSSProperties } from "react";

import { useAssetContext } from "./AssetContext";
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
 * Read-view card for `:::lattice-embed` blocks. Resolves the referenced
 * resource against the open page and shows a bounded preview by kind.
 */
export function LatticeEmbedView({ node }: NodeViewProps) {
  const { root, pagePath, onOpenEmbed } = useAssetContext();
  const resource = (node.attrs.resource as string) || "";
  const view = (node.attrs.view as string | null) ?? undefined;
  const height = (node.attrs.height as string | null) ?? undefined;
  const lines = (node.attrs.lines as string | null) ?? undefined;
  const missing = resource.trim().length === 0;

  const [preview, setPreview] = useState<EmbedPreviewResult | null>(null);
  const [loading, setLoading] = useState(false);
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
        { resource, view, height, lines },
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
  }, [height, lines, missing, pagePath, resource, root, view]);

  const previewKind = preview?.kind ?? "unknown";
  const KindIcon = kindIcon(previewKind);
  const heightStyle = parseEmbedHeight(height);
  const interactive = Boolean(onOpenEmbed && preview?.resolvedPath);

  const handleActivate = () => {
    if (!onOpenEmbed || !preview?.resolvedPath) return;
    onOpenEmbed(preview.resolvedPath);
  };

  return (
    <NodeViewWrapper className="page-embed-lattice">
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
            {missing ? "Embed" : `${KIND_LABEL[previewKind]} embed`}
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

            {preview?.unavailable ? (
              <p className="page-embed-lattice-status">{preview.unavailable}</p>
            ) : null}

            {!loading &&
            preview &&
            !preview.excerpt &&
            !preview.imageUrl &&
            preview.kind !== "pdf" &&
            preview.kind !== "data-app" ? (
              <p className="page-embed-lattice-resource">{preview.resolvedPath}</p>
            ) : null}
          </div>
        )}

        {!missing && preview?.resolvedPath ? (
          <p className="page-embed-lattice-path">{preview.resolvedPath}</p>
        ) : null}
      </div>
    </NodeViewWrapper>
  );
}
