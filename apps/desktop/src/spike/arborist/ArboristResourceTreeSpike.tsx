/**
 * Controlled react-arborist spike for the C1 catalog projection.
 *
 * Not mounted in production shell — evaluate behind a dev flag or story before
 * replacing `ResourceTree`.
 */
import { useMemo, type MouseEvent } from "react";
import { Tree, type NodeRendererProps } from "react-arborist";

import { fileTitle } from "../../controllers/useResourceController";
import { KindMark, KIND_LABELS } from "../../KindMark";
import { resourceTreeIcon } from "../../lib/resourceIcons";
import { RESOURCE_TREE_ROW_HEIGHT } from "../../lib/resourceTree";
import type { CatalogEntry } from "../../lib/resourceCatalog";
import type { Resource } from "../../types";
import {
  arboristCatalogSearchMatch,
  catalogToArboristForest,
  pathsForArboristMove,
  type ArboristCatalogNode,
} from "./catalogToArboristData";

export interface ArboristSpikeMutations {
  /** Semantic move — maps drag resource ids to `move_resources` paths. */
  onMoveByResourceId: (args: {
    dragResourceIds: readonly string[];
    parentResourceId: string | null;
    index: number;
  }) => void | Promise<void>;
  onRenameByResourceId: (args: { resourceId: string; title: string }) => void | Promise<void>;
}

export interface ArboristResourceTreeSpikeProps {
  catalog: ReadonlyMap<string, CatalogEntry>;
  selectedResourceIds: ReadonlySet<string>;
  onSelect: (detail: {
    resourceIds: ReadonlySet<string>;
    primary: Resource | null;
    open: boolean;
  }) => void;
  mutations: ArboristSpikeMutations;
  searchTerm?: string;
  width?: number | string;
  height?: number;
  className?: string;
}

const TREE_ICON_SIZE = 15;

function resourceFromNode(node: ArboristCatalogNode): Resource | null {
  if (node.isFolder) return null;
  return { path: node.path, kind: node.kind };
}

function SpikeRow({ node, style, dragHandle }: NodeRendererProps<ArboristCatalogNode>) {
  const data = node.data;
  const decision = data.isFolder ? null : resourceTreeIcon({ path: data.path, kind: data.kind });

  return (
    <div
      ref={dragHandle}
      className={
        "resource-item resource-tree-row resource-tree-row--arborist-spike"
        + (node.isSelected ? " resource-item-active" : "")
      }
      style={style}
      role="treeitem"
      aria-label={`${KIND_LABELS[data.kind]}: ${data.path}`}
      aria-selected={node.isSelected}
      title={data.path}
      onClick={(event: MouseEvent) => {
        if (event.metaKey || event.ctrlKey) node.selectMulti();
        else if (event.shiftKey) node.selectContiguous();
        else node.select();
        node.activate();
      }}
    >
      {decision?.type === "kind-mark" ? (
        <KindMark kind={decision.kind} size={TREE_ICON_SIZE} />
      ) : decision ? (
        <decision.Icon size={TREE_ICON_SIZE} weight="regular" className="resource-tree-icon" aria-hidden />
      ) : null}
      <span className="resource-path">{data.name}</span>
    </div>
  );
}

export function ArboristResourceTreeSpike({
  catalog,
  selectedResourceIds: _selectedResourceIds,
  onSelect,
  mutations,
  searchTerm = "",
  width = "100%",
  height = 480,
  className,
}: ArboristResourceTreeSpikeProps) {
  const data = useMemo(() => catalogToArboristForest(catalog), [catalog]);

  if (catalog.size === 0) {
    return (
      <div className="resource-list-empty">This folder is empty. Files you add appear here.</div>
    );
  }

  return (
    <Tree<ArboristCatalogNode>
      className={className ?? "resource-tree-arborist-spike"}
      data={data}
      idAccessor="resourceId"
      childrenAccessor="children"
      width={width}
      height={height}
      indent={16}
      rowHeight={RESOURCE_TREE_ROW_HEIGHT}
      overscanCount={8}
      openByDefault={false}
      searchTerm={searchTerm}
      searchMatch={(node, term) => arboristCatalogSearchMatch(node, term)}
      onSelect={(nodes) => {
        const resourceIds = new Set(
          nodes.filter((node) => !node.data.isFolder).map((node) => node.data.resourceId),
        );
        const primaryNode = nodes.find((node) => !node.data.isFolder) ?? null;
        onSelect({
          resourceIds,
          primary: primaryNode ? resourceFromNode(primaryNode.data) : null,
          open: resourceIds.size > 0,
        });
      }}
      onMove={({ dragIds, parentId, index }) => {
        if (!pathsForArboristMove(catalog, dragIds, parentId)) return;
        void mutations.onMoveByResourceId({
          dragResourceIds: dragIds,
          parentResourceId: parentId,
          index,
        });
      }}
      onRename={({ id, name }) => {
        const draft = name.trim();
        if (!draft) return;
        const entry = catalog.get(id);
        if (!entry || entry.kind === "folder") return;
        if (draft === fileTitle(entry.path)) return;
        void mutations.onRenameByResourceId({ resourceId: id, title: draft });
      }}
    >
      {SpikeRow}
    </Tree>
  );
}
