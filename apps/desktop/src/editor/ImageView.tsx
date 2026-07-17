import { invoke } from "@tauri-apps/api/core";
import { NodeViewWrapper } from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";
import { useEffect, useState } from "react";

import { useAssetContext } from "./AssetContext";
import { assetMimeType, isAbsoluteSrc, resolveWorkspaceAssetPath } from "./assets";
import { heavyEmbedPlaceholderStyle, useDeferredUntilVisible } from "./visibilityDeferred";

/**
 * Read-view image embed. Workspace-relative images are read through a
 * containment-checked Tauri command and rendered from a short-lived Blob URL.
 * This keeps arbitrary filesystem paths outside the webview's authority.
 *
 * Binary reads and decode are deferred until the embed is near the viewport
 * (ADR 0036); the ProseMirror node view stays mounted for editing correctness.
 */
export function ImageView({ node }: NodeViewProps) {
  const { root, pagePath } = useAssetContext();
  const src = node.attrs.src as string;
  const alt = (node.attrs.alt as string | null) ?? "";
  const title = (node.attrs.title as string | null) ?? undefined;
  const { ref, isVisible } = useDeferredUntilVisible();
  const [resolvedSrc, setResolvedSrc] = useState<string | null>(() =>
    !root || isAbsoluteSrc(src) ? src : null,
  );
  const [loadError, setLoadError] = useState<string | null>(null);
  const placeholderStyle = heavyEmbedPlaceholderStyle(node.attrs.width, node.attrs.height);

  useEffect(() => {
    if (!isVisible) return;

    if (!root || isAbsoluteSrc(src)) {
      setResolvedSrc(src);
      setLoadError(null);
      return;
    }

    const relPath = resolveWorkspaceAssetPath(pagePath, src);
    if (!relPath) return;
    let disposed = false;
    let objectUrl: string | null = null;
    setResolvedSrc(null);
    setLoadError(null);

    void invoke<ArrayBuffer | number[]>("read_binary_file", { root, relPath })
      .then((bytes) => {
        if (disposed) return;
        objectUrl = URL.createObjectURL(
          new Blob([bytes instanceof ArrayBuffer ? bytes : new Uint8Array(bytes)], {
            type: assetMimeType(relPath),
          }),
        );
        setResolvedSrc(objectUrl);
      })
      .catch((error) => {
        if (!disposed) setLoadError(error instanceof Error ? error.message : String(error));
      });

    return () => {
      disposed = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [isVisible, pagePath, root, src]);

  const showImage = isVisible && resolvedSrc;

  return (
    <NodeViewWrapper as="span" className="page-embed-image">
      <span ref={ref} className="page-embed-image-mount">
        {showImage ? (
          <img src={resolvedSrc} alt={alt} title={title} />
        ) : (
          <span
            className="page-embed-image-placeholder"
            style={placeholderStyle}
            role={loadError ? "alert" : "status"}
          >
            {loadError
              ? `Could not load ${alt || src}`
              : isVisible
                ? `Loading ${alt || src}…`
                : alt || src}
          </span>
        )}
      </span>
    </NodeViewWrapper>
  );
}
