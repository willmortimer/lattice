import { useEffect, useMemo, useRef, useState, type DragEvent } from "react";

import { fileTitle } from "./controllers/useResourceController";
import { KindMark, KIND_LABELS } from "./KindMark";
import { readResourceDragPayload, writeResourceDragPayload } from "./lib/resourceDrag";
import { folderTreeIcon, resourceTreeIcon } from "./lib/resourceIcons";
import {
  buildResourceTree,
  flattenVisibleTree,
  RESOURCE_TREE_ROW_HEIGHT,
  type FlatRow,
} from "./lib/resourceTree";
import { validateMoveResource } from "./lib/treeOps";
import type { Resource } from "./types";

interface ResourceTreeProps {
  resources: readonly Resource[];
  selectedPath: string | null;
  onSelect: (resource: Resource) => void;
  onResourceContextMenu?: (resource: Resource) => void;
  onFolderContextMenu?: (folderPath: string) => void;
  onRename?: (resource: Resource, title: string) => Promise<void>;
  onMoveToFolder?: (from: string, toDir: string) => void;
  renameRequest?: { path: string; token: number } | null;
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
const OVERSCAN = 8;

function ResourceTreeRowIcon({ resource }: { resource: Resource }) {
  const decision = resourceTreeIcon(resource);
  if (decision.type === "kind-mark") {
    return <KindMark kind={decision.kind} size={TREE_ICON_SIZE} />;
  }
  const Icon = decision.Icon;
  return <Icon size={TREE_ICON_SIZE} weight="regular" className="resource-tree-icon" aria-hidden />;
}

function acceptsResourceDrop(
  event: DragEvent,
  resources: readonly Resource[],
  from: string,
  toDir: string,
): boolean {
  if (!event.dataTransfer.types.includes("application/x-lattice-resource")) return false;
  return validateMoveResource(from, toDir, resources).ok;
}

function useResourceListScroll() {
  const rootRef = useRef<HTMLDivElement>(null);
  const scrollParentRef = useRef<HTMLElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;

    const parent = root.closest(".resource-list");
    if (!(parent instanceof HTMLElement)) return;

    scrollParentRef.current = parent;

    const sync = () => {
      setScrollTop(parent.scrollTop);
      setViewportHeight(parent.clientHeight);
    };

    sync();
    parent.addEventListener("scroll", sync, { passive: true });
    const observer = new ResizeObserver(sync);
    observer.observe(parent);

    return () => {
      parent.removeEventListener("scroll", sync);
      observer.disconnect();
      if (scrollParentRef.current === parent) scrollParentRef.current = null;
    };
  }, []);

  return { rootRef, scrollParentRef, scrollTop, viewportHeight };
}

/**
 * Collapsible folder tree over a flat resource listing — replaces the
 * former flat `resource-list`. Folders group by path segment (sorted
 * before files, both alphabetically within a level; see
 * `lib/resourceTree`). Collapse state persists per workspace in the
 * Lattice profile when `workspaceKey` and change handlers are provided.
 *
 * Visible rows are flattened and windowed so large workspaces only mount
 * rows near the `.resource-list` scroll viewport.
 */
export function ResourceTree({
  resources,
  selectedPath,
  onSelect,
  onResourceContextMenu,
  onFolderContextMenu,
  onRename,
  onMoveToFolder,
  renameRequest,
  revealPath,
  directoryPurposes,
  workspaceKey: _workspaceKey,
  collapsedPaths,
  onCollapsedPathsChange,
}: ResourceTreeProps) {
  const [localCollapsed, setLocalCollapsed] = useState<ReadonlySet<string>>(() => new Set());
  const [editingPath, setEditingPath] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [dropTargetPath, setDropTargetPath] = useState<string | null>(null);
  const collapsed = collapsedPaths ?? localCollapsed;
  const { rootRef, scrollParentRef, scrollTop, viewportHeight } = useResourceListScroll();

  const tree = useMemo(() => buildResourceTree(resources), [resources]);
  const rows = useMemo(() => flattenVisibleTree(tree, collapsed), [collapsed, tree]);

  const firstVisible = Math.max(0, Math.floor(scrollTop / RESOURCE_TREE_ROW_HEIGHT) - OVERSCAN);
  const lastVisible = Math.min(
    rows.length,
    Math.ceil((scrollTop + viewportHeight) / RESOURCE_TREE_ROW_HEIGHT) + OVERSCAN,
  );
  const visibleRows = rows.slice(firstVisible, lastVisible);

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

  useEffect(() => {
    if (!revealPath) return;
    const index = rows.findIndex((row) => row.type === "file" && row.resource.path === revealPath);
    if (index < 0) return;

    const parent = scrollParentRef.current;
    if (!parent) return;

    const rowTop = index * RESOURCE_TREE_ROW_HEIGHT;
    const rowBottom = rowTop + RESOURCE_TREE_ROW_HEIGHT;
    if (rowTop < parent.scrollTop) {
      parent.scrollTop = rowTop;
    } else if (rowBottom > parent.scrollTop + parent.clientHeight) {
      parent.scrollTop = rowBottom - parent.clientHeight;
    }
  }, [revealPath, rows, scrollParentRef]);

  useEffect(() => {
    if (!renameRequest) return;
    setEditingPath(renameRequest.path);
    setRenameDraft(fileTitle(renameRequest.path));
  }, [renameRequest]);

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

  function beginRename(resource: Resource) {
    setEditingPath(resource.path);
    setRenameDraft(fileTitle(resource.path));
  }

  async function commitRename(resource: Resource) {
    const draft = renameDraft.trim();
    setEditingPath(null);
    if (!draft || draft === fileTitle(resource.path)) return;
    await onRename?.(resource, draft);
  }

  function cancelRename(resource: Resource) {
    setEditingPath(null);
    setRenameDraft(fileTitle(resource.path));
  }

  function handleFolderDragOver(event: DragEvent, folderPath: string) {
    const payload = readResourceDragPayload(event.dataTransfer);
    if (!payload) return;
    if (!acceptsResourceDrop(event, resources, payload.path, folderPath)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    setDropTargetPath(folderPath);
  }

  function handleFolderDrop(event: DragEvent, folderPath: string) {
    event.preventDefault();
    setDropTargetPath(null);
    const payload = readResourceDragPayload(event.dataTransfer);
    if (!payload) return;
    if (!validateMoveResource(payload.path, folderPath, resources).ok) return;
    onMoveToFolder?.(payload.path, folderPath);
  }

  function renderRow(row: FlatRow, index: number) {
    const indent = INDENT_BASE_PX + row.depth * INDENT_STEP_PX;
    const style = {
      top: index * RESOURCE_TREE_ROW_HEIGHT,
      paddingLeft: indent,
    };

    if (row.type === "file") {
      const { resource } = row;
      const isEditing = editingPath === resource.path;
      return (
        <button
          key={`file:${resource.path}`}
          className={
            "resource-item resource-tree-row"
            + (selectedPath === resource.path ? " resource-item-active" : "")
          }
          style={style}
          aria-label={`${KIND_LABELS[resource.kind]}: ${resource.path}`}
          title={resource.path}
          draggable={!isEditing}
          onDragStart={(event) => {
            writeResourceDragPayload(event.dataTransfer, resource);
          }}
          onClick={() => onSelect(resource)}
          onContextMenu={(event) => {
            event.preventDefault();
            onResourceContextMenu?.(resource);
          }}
        >
          <ResourceTreeRowIcon resource={resource} />
          {isEditing ? (
            <input
              className="tree-rename-input"
              value={renameDraft}
              autoFocus
              aria-label={`Rename ${resource.path}`}
              onClick={(event) => event.stopPropagation()}
              onChange={(event) => setRenameDraft(event.target.value)}
              onBlur={() => void commitRename(resource)}
              onKeyDown={(event) => {
                if (event.key === "Enter") void commitRename(resource);
                if (event.key === "Escape") cancelRename(resource);
              }}
            />
          ) : (
            <span
              className="resource-path"
              onDoubleClick={(event) => {
                event.stopPropagation();
                beginRename(resource);
              }}
            >
              {row.name}
            </span>
          )}
        </button>
      );
    }

    if (row.type === "empty-folder") {
      return (
        <div
          key={`empty:${row.path}`}
          className="resource-list-empty resource-tree-empty-row resource-tree-row"
          style={style}
        >
          {emptyFolderHint(row.path)}
        </div>
      );
    }

    const isCollapsed = collapsed.has(row.path);
    const FolderIcon = folderTreeIcon(isCollapsed);
    return (
      <button
        key={`folder:${row.path}`}
        className={
          "tree-folder-row resource-tree-row"
          + (dropTargetPath === row.path ? " tree-folder-row-drop-target" : "")
        }
        style={style}
        onClick={() => toggle(row.path)}
        aria-expanded={!isCollapsed}
        onContextMenu={(event) => {
          event.preventDefault();
          onFolderContextMenu?.(row.path);
        }}
        onDragOver={(event) => handleFolderDragOver(event, row.path)}
        onDragLeave={() => {
          if (dropTargetPath === row.path) setDropTargetPath(null);
        }}
        onDrop={(event) => handleFolderDrop(event, row.path)}
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
        <span className="tree-folder-name">{row.name}</span>
      </button>
    );
  }

  return (
    <div ref={rootRef} className="resource-tree-virtual">
      <div
        className="resource-tree-virtual-spacer"
        style={{ height: rows.length * RESOURCE_TREE_ROW_HEIGHT }}
      >
        {visibleRows.map((row, sliceIndex) => renderRow(row, firstVisible + sliceIndex))}
      </div>
    </div>
  );
}
