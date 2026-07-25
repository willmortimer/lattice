export type {
  AgentAnchorAdapter,
  AnchorHighlightPurpose,
  AnchorRevealBehavior,
  DatasetRegionAnchorAdapter,
  MarkdownBlockAnchorAdapter,
  WorkspaceAnchorKind,
} from "./types";

export {
  blockRangesForDocument,
  resolveBlockRange,
  structuralBlockId,
  type BlockRange,
  type StructuralBlockKind,
} from "./blockIds";

export {
  AgentAnchorHighlight,
  agentAnchorHighlightKey,
  createAgentAnchorHighlightPlugin,
  type AgentAnchorHighlightEntry,
  type AgentAnchorHighlightState,
} from "./AgentAnchorHighlight";

export {
  clearAnchorAdapters,
  getAnchorAdapter,
  getAnchorAdapterFor,
  registerAnchorAdapter,
} from "./registry";

export { createMarkdownBlockAdapter } from "./tiptapAdapter";
export {
  createDatasetRegionAdapter,
  type DatasetRegionSurfaceHandle,
} from "./glideAdapter";

export { registerDatasetAnchorSurface, registerPageAnchorSurface } from "./surfaces";
