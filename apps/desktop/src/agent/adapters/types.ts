import type {
  DatasetRegionAnchor,
  MarkdownBlockAnchor,
  WorkspaceAnchor,
} from "@lattice/agent-protocol";

export type AnchorRevealBehavior = "peek" | "reveal" | "follow";
export type AnchorHighlightPurpose = "attention" | "evidence" | "warning" | "change";

export type WorkspaceAnchorKind = WorkspaceAnchor["kind"];

export interface AgentAnchorAdapter<TAnchor extends WorkspaceAnchor = WorkspaceAnchor> {
  kind: TAnchor["kind"];
  resourceId: string;
  reveal(anchor: TAnchor, behavior: AnchorRevealBehavior): Promise<void>;
  highlight(
    anchor: TAnchor,
    options: { overlayId: string; purpose: AnchorHighlightPurpose },
  ): () => void;
  getScreenRect?(anchor: TAnchor): DOMRect | null;
}

export type MarkdownBlockAnchorAdapter = AgentAnchorAdapter<MarkdownBlockAnchor>;
export type DatasetRegionAnchorAdapter = AgentAnchorAdapter<DatasetRegionAnchor>;
