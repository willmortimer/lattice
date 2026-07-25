import type { Editor } from "@tiptap/core";

import { registerAnchorAdapter } from "./registry";
import { createDatasetRegionAdapter, type DatasetRegionSurfaceHandle } from "./glideAdapter";
import { createMarkdownBlockAdapter } from "./tiptapAdapter";

export function registerPageAnchorSurface(resourceId: string, editor: Editor): () => void {
  return registerAnchorAdapter(createMarkdownBlockAdapter(editor, resourceId));
}

export function registerDatasetAnchorSurface(handle: DatasetRegionSurfaceHandle): () => void {
  return registerAnchorAdapter(createDatasetRegionAdapter(handle));
}
