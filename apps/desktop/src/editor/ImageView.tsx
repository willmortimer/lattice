import { NodeViewWrapper } from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";

import { useAssetContext } from "./AssetContext";
import { resolveEmbedSrc } from "./assets";

/**
 * Read-view image embed: resolves a page-relative Markdown image `src`
 * through the Tauri asset protocol (`resolveEmbedSrc`), scoped to the
 * workspace root and the open page's own directory.
 */
export function ImageView({ node }: NodeViewProps) {
  const { root, pagePath } = useAssetContext();
  const src = node.attrs.src as string;
  const alt = (node.attrs.alt as string | null) ?? "";
  const title = (node.attrs.title as string | null) ?? undefined;

  return (
    <NodeViewWrapper as="span" className="page-embed-image">
      <img src={resolveEmbedSrc(root, pagePath, src)} alt={alt} title={title} />
    </NodeViewWrapper>
  );
}
