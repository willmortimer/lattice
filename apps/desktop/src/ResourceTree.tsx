import { useState } from "react";
import type { ReactNode } from "react";

import { KindMark, KIND_LABELS } from "./KindMark";
import { buildResourceTree, type TreeNode } from "./lib/resourceTree";
import type { Resource } from "./types";

interface ResourceTreeProps {
  resources: readonly Resource[];
  selectedPath: string | null;
  onSelect: (resource: Resource) => void;
}

const INDENT_BASE_PX = 9;
const INDENT_STEP_PX = 16;

/**
 * Collapsible folder tree over a flat resource listing — replaces the
 * former flat `resource-list`. Folders group by path segment (sorted
 * before files, both alphabetically within a level; see
 * `lib/resourceTree`); collapsing one is purely local UI state, not
 * persisted, and defaults to fully expanded so nothing already open in
 * the old flat list disappears on first render.
 */
export function ResourceTree({ resources, selectedPath, onSelect }: ResourceTreeProps) {
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(() => new Set());

  if (resources.length === 0) {
    return (
      <div className="resource-list-empty">This folder is empty. Files you add appear here.</div>
    );
  }

  function toggle(path: string) {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  function renderNode(node: TreeNode, depth: number): ReactNode {
    const indent = INDENT_BASE_PX + depth * INDENT_STEP_PX;

    if (node.type === "file") {
      const { resource } = node;
      return (
        <button
          key={resource.path}
          className={
            "resource-item" + (selectedPath === resource.path ? " resource-item-active" : "")
          }
          style={{ paddingLeft: indent }}
          aria-label={`${KIND_LABELS[resource.kind]}: ${resource.path}`}
          title={resource.path}
          onClick={() => onSelect(resource)}
        >
          <KindMark kind={resource.kind} />
          <span className="resource-path">{node.name}</span>
        </button>
      );
    }

    const isCollapsed = collapsed.has(node.path);
    return (
      <div key={node.path} className="tree-folder">
        <button
          className="tree-folder-row"
          style={{ paddingLeft: indent }}
          onClick={() => toggle(node.path)}
          aria-expanded={!isCollapsed}
        >
          <span
            className={"tree-chevron" + (isCollapsed ? "" : " tree-chevron-open")}
            aria-hidden="true"
          />
          <span className="tree-folder-name">{node.name}</span>
        </button>
        {!isCollapsed && (
          <div className="tree-children">
            {node.children.map((child) => renderNode(child, depth + 1))}
          </div>
        )}
      </div>
    );
  }

  return <>{buildResourceTree(resources).map((n) => renderNode(n, 0))}</>;
}
