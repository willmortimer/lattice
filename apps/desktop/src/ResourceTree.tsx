import { useEffect, useState } from "react";
import type { ReactNode } from "react";

import { KindMark, KIND_LABELS } from "./KindMark";
import { writeResourceDragPayload } from "./lib/resourceDrag";
import { folderTreeIcon, resourceTreeIcon } from "./lib/resourceIcons";
import { buildResourceTree, type TreeNode } from "./lib/resourceTree";
import type { Resource } from "./types";

interface ResourceTreeProps {
  resources: readonly Resource[];
  selectedPath: string | null;
  onSelect: (resource: Resource) => void;
  onContextMenu?: (resource: Resource) => void;
  revealPath?: string | null;
  /** Optional path → purpose hints from the active template catalog. */
  directoryPurposes?: Readonly<Record<string, string>>;
  /** Workspace id used to load/save collapsed folder paths in the profile. */
  workspaceKey?: string | null;
  collapsedPaths?: ReadonlySet<string>;
  onCollapsedPathsChange?: (paths: ReadonlySet<string>) => void;
}

const INDENT_BASE_PX = 9;
const INDENT_STEP_PX = 16;
const TREE_ICON_SIZE = 15;
const FOLDER_ICON_SIZE = 14;

function ResourceTreeRowIcon({ resource }: { resource: Resource }) {
  const decision = resourceTreeIcon(resource);
  if (decision.type === "kind-mark") {
    return <KindMark kind={decision.kind} size={TREE_ICON_SIZE} />;
  }
  const Icon = decision.Icon;
  return <Icon size={TREE_ICON_SIZE} weight="regular" className="resource-tree-icon" aria-hidden />;
}

/**
 * Collapsible folder tree over a flat resource listing — replaces the
 * former flat `resource-list`. Folders group by path segment (sorted
 * before files, both alphabetically within a level; see
 * `lib/resourceTree`). Collapse state persists per workspace in the
 * Lattice profile when `workspaceKey` and change handlers are provided.
 */
export function ResourceTree({
  resources,
  selectedPath,
  onSelect,
  onContextMenu,
  revealPath,
  directoryPurposes,
  workspaceKey: _workspaceKey,
  collapsedPaths,
  onCollapsedPathsChange,
}: ResourceTreeProps) {
  const [localCollapsed, setLocalCollapsed] = useState<ReadonlySet<string>>(() => new Set());
  const collapsed = collapsedPaths ?? localCollapsed;

  function updateCollapsed(updater: (previous: ReadonlySet<string>) => ReadonlySet<string>) {
    const previous = collapsedPaths ?? localCollapsed;
    const next = updater(previous);
    if (onCollapsedPathsChange) onCollapsedPathsChange(next);
    else setLocalCollapsed(next);
  }

  useEffect(() => {
    if (!revealPath) return;
    const parts = revealPath.replace(/\/$/, "").split("/");
    const ancestors = parts.slice(0, -1).map((_, index) => parts.slice(0, index + 1).join("/"));
    updateCollapsed((previous) => {
      const next = new Set(previous);
      ancestors.forEach((path) => next.delete(path));
      return next;
    });
  }, [revealPath]);

  if (resources.length === 0) {
    return (
      <div className="resource-list-empty">This folder is empty. Files you add appear here.</div>
    );
  }

  function toggle(path: string) {
    updateCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  function emptyFolderHint(path: string): string {
    return directoryPurposes?.[path] ?? "This folder is empty. Files you add appear here.";
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
          draggable
          onDragStart={(event) => {
            writeResourceDragPayload(event.dataTransfer, resource);
          }}
          onClick={() => onSelect(resource)}
          onContextMenu={() => onContextMenu?.(resource)}
        >
          <ResourceTreeRowIcon resource={resource} />
          <span className="resource-path">{node.name}</span>
        </button>
      );
    }

    const isCollapsed = collapsed.has(node.path);
    const FolderIcon = folderTreeIcon(isCollapsed);
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
          <FolderIcon
            size={FOLDER_ICON_SIZE}
            weight="regular"
            className="resource-tree-folder-icon"
            aria-hidden
          />
          <span className="tree-folder-name">{node.name}</span>
        </button>
        {!isCollapsed && (
          <div className="tree-children">
            {node.children.length === 0 ? (
              <div
                className="resource-list-empty"
                style={{ paddingLeft: indent + INDENT_STEP_PX }}
              >
                {emptyFolderHint(node.path)}
              </div>
            ) : (
              node.children.map((child) => renderNode(child, depth + 1))
            )}
          </div>
        )}
      </div>
    );
  }

  return <>{buildResourceTree(resources).map((n) => renderNode(n, 0))}</>;
}
