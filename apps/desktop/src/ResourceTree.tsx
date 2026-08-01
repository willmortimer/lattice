import { useEffect, useMemo, useRef, useState, type DragEvent, type MouseEvent } from "react";

import { fileTitle } from "./controllers/useResourceController";
import { KindMark, KIND_LABELS } from "./KindMark";
import { hasLatticeResourceDrag, readResourceDragPayload, writeResourceDragPayload } from "./lib/resourceDrag";
import { folderTreeIcon, resourceTreeIcon } from "./lib/resourceIcons";
import {
  pathsForResourceIds,
  type CatalogDelta,
  type CatalogEntry,
} from "./lib/resourceCatalog";
import {
  applyCatalogDeltaToForest,
  buildResourceTreeFromCatalog,
  flattenVisibleTree,
  RESOURCE_TREE_ROW_HEIGHT,
  type FlatRow,
  type TreeNode,
} from "./lib/resourceTree";
import { nextTreeSelection, resourceIdsForTreeDrag, type TreeSelectMode } from "./lib/treeSelection";
import { validateMoveResources } from "./lib/treeOps";
import type { Resource } from "./types";

interface ResourceTreeProps {
  /** Id-keyed workspace catalog (preferred over flat resources). */
  catalog: ReadonlyMap<string, CatalogEntry>;
  /** Latest catalog-delta applied to `catalog` (null on full replace/seed). */
  catalogDelta?: CatalogDelta | null;
  selectedResourceIds: ReadonlySet<string>;
  onTreeSelect: (detail: {
    resourceIds: ReadonlySet<string>;
    primary: Resource | null;
    open: boolean;
  }) => void;
  onResourceContextMenu?: (resource: Resource) => void;
  onFolderContextMenu?: (folderPath: string) => void;
  onRename?: (resource: Resource, title: string) => Promise<void>;
  onMoveToFolder?: (fromPaths: readonly string[], toDir: string) => void;
  renameRequest?: { path: string; token: number } | null;
  revealPath?: string | null;
  /** Optional path → purpose hints from the active template catalog. */
  directoryPurposes?: Readonly<Record<string, string>>;
  /** Workspace id used to load/save collapsed folder paths in the profile. */
  workspaceKey?: string | null;
  collapsedPaths?: ReadonlySet<string>;
  onCollapsedPathsChange?: (paths: ReadonlySet<string>) => void;
  /** Browser demo: highlight and target the last clicked folder row. */
  activeFolderPath?: string | null;
  onActiveFolderChange?: (folderPath: string) => void;
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

function selectModeFromEvent(event: MouseEvent): TreeSelectMode {
  if (event.shiftKey) return "range";
  if (event.metaKey || event.ctrlKey) return "toggle";
  return "replace";
}

function acceptsResourceDrop(
  event: DragEvent,
  resources: readonly Resource[],
  fromPaths: readonly string[],
  toDir: string,
): boolean {
  if (fromPaths.length === 0) return false;
  // In-app dragstart always sets fromPaths via dragPathsRef. Synthetic DnD
  // (Tauri e2e dragAndDrop) often leaves DataTransfer.types empty even after
  // setData — still accept when we own the drag paths.
  if (
    event.dataTransfer.types.length > 0 &&
    !hasLatticeResourceDrag(event.dataTransfer)
  ) {
    return false;
  }
  return validateMoveResources(fromPaths, toDir, resources).ok;
}

function useResourceListScroll() {
  const rootRef = useRef<HTMLDivElement>(null);
  const scrollParentRef = useRef<HTMLElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);
  const rafRef = useRef<number | null>(null);

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;

    const parent = root.closest(".resource-list");
    if (!(parent instanceof HTMLElement)) return;

    scrollParentRef.current = parent;

    const syncNow = () => {
      setScrollTop(parent.scrollTop);
      setViewportHeight(parent.clientHeight);
    };

    const sync = () => {
      if (rafRef.current !== null) return;
      rafRef.current = window.requestAnimationFrame(() => {
        rafRef.current = null;
        syncNow();
      });
    };

    syncNow();
    parent.addEventListener("scroll", sync, { passive: true });
    const observer = new ResizeObserver(sync);
    observer.observe(parent);

    return () => {
      parent.removeEventListener("scroll", sync);
      observer.disconnect();
      if (rafRef.current !== null) {
        window.cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
      if (scrollParentRef.current === parent) scrollParentRef.current = null;
    };
  }, []);

  return { rootRef, scrollParentRef, scrollTop, viewportHeight };
}

/**
 * Collapsible folder tree over the workspace catalog — keyed by resourceId
 * for selection so renames and synthetic→UUID upgrades keep identity.
 *
 * Forest updates prefer incremental `catalog-delta` patches; replace/seed
 * rebuilds from the catalog map (not a path-derived Resource[] scan).
 *
 * Multi-select: plain click replaces; ⌘/Ctrl-click toggles; Shift-click
 * selects a contiguous range of visible file rows.
 */
export function ResourceTree({
  catalog,
  catalogDelta = null,
  selectedResourceIds,
  onTreeSelect,
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
  activeFolderPath,
  onActiveFolderChange,
}: ResourceTreeProps) {
  const [localCollapsed, setLocalCollapsed] = useState<ReadonlySet<string>>(() => new Set());
  const [editingPath, setEditingPath] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [dropTargetPath, setDropTargetPath] = useState<string | null>(null);
  const selectionAnchorRef = useRef<string | null>(null);
  const selectedResourceIdsRef = useRef(selectedResourceIds);
  /** Paths captured at dragstart — dragover cannot read DataTransfer payloads. */
  const dragPathsRef = useRef<string[] | null>(null);
  selectedResourceIdsRef.current = selectedResourceIds;
  const collapsed = collapsedPaths ?? localCollapsed;
  const { rootRef, scrollParentRef, scrollTop, viewportHeight } = useResourceListScroll();

  const appliedCatalogRef = useRef<ReadonlyMap<string, CatalogEntry> | null>(null);
  const forestRef = useRef<TreeNode[]>([]);
  const [forest, setForest] = useState<TreeNode[]>([]);

  useEffect(() => {
    // Skip when React re-runs with the same catalog map instance.
    if (appliedCatalogRef.current === catalog) return;
    const previous = appliedCatalogRef.current;
    let nextForest: TreeNode[];
    if (
      !previous
      || !catalogDelta
      || catalogDelta.type === "replace"
      || catalogDelta.type === "reorder"
    ) {
      nextForest = buildResourceTreeFromCatalog(catalog);
    } else {
      nextForest = applyCatalogDeltaToForest(
        forestRef.current,
        previous,
        catalogDelta,
        catalog,
      ).forest;
    }
    forestRef.current = nextForest;
    appliedCatalogRef.current = catalog;
    setForest(nextForest);
  }, [catalog, catalogDelta]);

  const resources = useMemo(
    () => [...catalog.values()].map((entry) => ({ path: entry.path, kind: entry.kind })),
    [catalog],
  );
  const rows = useMemo(() => flattenVisibleTree(forest, collapsed), [collapsed, forest]);
  const visibleResourceIds = useMemo(
    () => rows.filter((row) => row.type === "file").map((row) => row.resourceId),
    [rows],
  );

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

  if (catalog.size === 0) {
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

  function handleFileClick(event: MouseEvent, resourceId: string, resource: Resource) {
    const mode = selectModeFromEvent(event);
    const result = nextTreeSelection({
      previous: selectedResourceIds,
      anchor: selectionAnchorRef.current,
      clicked: resourceId,
      visibleResourceIds,
      mode,
    });
    selectionAnchorRef.current = result.anchor;
    const open = mode !== "toggle" || result.selected.has(resourceId);
    onTreeSelect({
      resourceIds: result.selected,
      primary: result.selected.has(resourceId) ? resource : null,
      open,
    });
  }

  function dragPathsFor(resourceId: string): string[] {
    const ids = resourceIdsForTreeDrag(resourceId, selectedResourceIdsRef.current);
    return pathsForResourceIds(catalog, ids);
  }

  function handleFolderDragOver(event: DragEvent, folderPath: string) {
    const fromPaths = dragPathsRef.current;
    if (!fromPaths) return;
    if (!acceptsResourceDrop(event, resources, fromPaths, folderPath)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    setDropTargetPath(folderPath);
  }

  function handleFolderDrop(event: DragEvent, folderPath: string) {
    event.preventDefault();
    setDropTargetPath(null);
    const payload = readResourceDragPayload(event.dataTransfer);
    const fromPaths = payload
      ? (() => {
          const id = [...catalog.values()].find((entry) => entry.path === payload.path)?.resourceId;
          return id ? dragPathsFor(id) : dragPathsRef.current;
        })()
      : dragPathsRef.current;
    dragPathsRef.current = null;
    if (!fromPaths || fromPaths.length === 0) return;
    if (!validateMoveResources(fromPaths, folderPath, resources).ok) return;
    onMoveToFolder?.(fromPaths, folderPath);
  }

  function renderRow(row: FlatRow, index: number) {
    const indent = INDENT_BASE_PX + row.depth * INDENT_STEP_PX;
    const style = {
      top: index * RESOURCE_TREE_ROW_HEIGHT,
      paddingLeft: indent,
    };

    if (row.type === "file") {
      const { resource, resourceId } = row;
      const isEditing = editingPath === resource.path;
      const isSelected = selectedResourceIds.has(resourceId);
      return (
        <button
          key={`file:${resourceId}`}
          className={
            "resource-item resource-tree-row"
            + (isSelected ? " resource-item-active" : "")
          }
          style={style}
          aria-label={`${KIND_LABELS[resource.kind]}: ${resource.path}`}
          aria-selected={isSelected}
          title={resource.path}
          draggable={!isEditing}
          onDragStart={(event) => {
            writeResourceDragPayload(event.dataTransfer, resource);
            dragPathsRef.current = dragPathsFor(resourceId);
          }}
          onDragEnd={() => {
            dragPathsRef.current = null;
            setDropTargetPath(null);
          }}
          onClick={(event) => handleFileClick(event, resourceId, resource)}
          onContextMenu={(event) => {
            event.preventDefault();
            if (!selectedResourceIds.has(resourceId)) {
              selectionAnchorRef.current = resourceId;
              onTreeSelect({
                resourceIds: new Set([resourceId]),
                primary: resource,
                open: true,
              });
            }
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
          key={`empty:${row.resourceId}`}
          className="resource-list-empty resource-tree-empty-row resource-tree-row"
          style={style}
        >
          {emptyFolderHint(row.path)}
        </div>
      );
    }

    const isCollapsed = collapsed.has(row.path);
    const FolderIcon = folderTreeIcon(isCollapsed);
    const isActiveFolder = activeFolderPath === row.path;
    return (
      <button
        key={`folder:${row.resourceId}`}
        className={
          "tree-folder-row resource-tree-row"
          + (isActiveFolder ? " tree-folder-row-active" : "")
          + (dropTargetPath === row.path ? " tree-folder-row-drop-target" : "")
        }
        style={style}
        onClick={() => {
          toggle(row.path);
          onActiveFolderChange?.(row.path);
        }}
        aria-label={`${KIND_LABELS.folder}: ${row.path}`}
        aria-expanded={!isCollapsed}
        aria-current={isActiveFolder ? "location" : undefined}
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
    <div ref={rootRef} className="resource-tree-virtual" role="tree" aria-multiselectable="true">
      <div
        className="resource-tree-virtual-spacer"
        style={{ height: rows.length * RESOURCE_TREE_ROW_HEIGHT }}
      >
        {visibleRows.map((row, sliceIndex) => renderRow(row, firstVisible + sliceIndex))}
      </div>
    </div>
  );
}
