import { invoke } from "@tauri-apps/api/core";
import { NodeViewWrapper } from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";
import { useEffect, useState } from "react";

import { useAssetContext } from "./AssetContext";
import { assetMimeType, isAbsoluteSrc, resolveWorkspaceAssetPath } from "./assets";

/**
 * Read-view image embed. Workspace-relative images are read through a
 * containment-checked Tauri command and rendered from a short-lived Blob URL.
 * This keeps arbitrary filesystem paths outside the webview's authority.
 */
export function ImageView({ node }: NodeViewProps) {
  const { root, pagePath } = useAssetContext();
  const src = node.attrs.src as string;
  const alt = (node.attrs.alt as string | null) ?? "";
  const title = (node.attrs.title as string | null) ?? undefined;
  const [resolvedSrc, setResolvedSrc] = useState<string | null>(() =>
    !root || isAbsoluteSrc(src) ? src : null,
  );
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
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
  }, [pagePath, root, src]);

  return (
    <NodeViewWrapper as="span" className="page-embed-image">
      {resolvedSrc ? (
        <img src={resolvedSrc} alt={alt} title={title} />
      ) : (
        <span className="page-embed-image-placeholder" role={loadError ? "alert" : "status"}>
          {loadError ? `Could not load ${alt || src}` : `Loading ${alt || src}…`}
        </span>
      )}
    </NodeViewWrapper>
  );
}
