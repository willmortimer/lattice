import { emitTo } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useMemo, useState } from "react";

import {
  defaultsToCellValues,
  listPackageActions,
  type ActionScope,
  type ActionSummary,
} from "./actions";
import type { CellValue, DataAppSnapshot, DataColumn, DataRow } from "./types";

const STALE_REVISION_PREFIX = "STALE_REVISION:";

function isStaleRevisionError(message: string): boolean {
  return message.startsWith(STALE_REVISION_PREFIX);
}

interface DataActionsMenuProps {
  root: string;
  relPath: string;
  table: string;
  columns: DataColumn[];
  scope: ActionScope;
  row?: DataRow | null;
  activeView: string;
  rowFetchLimit: number;
  packageRevision: string;
  busy: boolean;
  readOnly: boolean;
  demo?: boolean;
  menuLabel?: string;
  onOpenForm: (formName: string) => void | Promise<void>;
  onSnapshot: (snapshot: DataAppSnapshot) => void;
  onStale: () => void;
  onError: (message: string | null) => void;
}

export function DataActionsMenu({
  root,
  relPath,
  table,
  columns,
  scope,
  row,
  activeView,
  rowFetchLimit,
  packageRevision,
  busy,
  readOnly,
  demo = false,
  menuLabel = "Actions",
  onOpenForm,
  onSnapshot,
  onStale,
  onError,
}: DataActionsMenuProps) {
  const [actions, setActions] = useState<ActionSummary[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoadError(null);
    void listPackageActions({ root, relPath, demo })
      .then((loaded) => {
        if (!cancelled) setActions(loaded);
      })
      .catch((err) => {
        if (!cancelled) {
          setActions([]);
          setLoadError(String(err));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [demo, relPath, root]);

  const visibleActions = useMemo(
    () =>
      actions.filter(
        (action) => action.table === table && action.scope === scope,
      ),
    [actions, scope, table],
  );

  const runAction = useCallback(
    async (action: ActionSummary) => {
      if (readOnly || busy) return;
      onError(null);
      setOpen(false);

      switch (action.action.type) {
        case "insert_record": {
          if (action.action.form) {
            await onOpenForm(action.action.form);
            return;
          }
          if (demo) {
            onError("Insert actions are not available in browser demo mode without a form.");
            return;
          }
          const values = defaultsToCellValues(action.action.defaults, columns);
          const result = await invoke<{ id: string; revision: string }>("insert_record", {
            root,
            relPath,
            table: action.table,
            values,
          });
          const fresh = await invoke<DataAppSnapshot>("open_data_app", {
            root,
            relPath,
            viewName: activeView,
            limit: rowFetchLimit,
            offset: 0,
          });
          onSnapshot({
            ...fresh,
            package_revision: result.revision,
          });
          return;
        }
        case "update_field": {
          const updateAction = action.action;
          if (!row) {
            onError("Select a row before running this action.");
            return;
          }
          if (demo) {
            onError("Row update actions are not available in browser demo mode.");
            return;
          }
          const column = columns.find((entry) => entry.name === updateAction.field);
          if (!column) {
            onError(`Unknown field ${updateAction.field}.`);
            return;
          }
          const values: Record<string, CellValue> = {
            [updateAction.field]: defaultsToCellValues(
              { [updateAction.field]: updateAction.value },
              columns,
            )[updateAction.field],
          };
          const revision = await invoke<string>("update_record", {
            root,
            relPath,
            table: action.table,
            id: row.id,
            values,
            baseRevision: packageRevision,
          });
          const fresh = await invoke<DataAppSnapshot>("open_data_app", {
            root,
            relPath,
            viewName: activeView,
            limit: rowFetchLimit,
            offset: 0,
          });
          onSnapshot({
            ...fresh,
            package_revision: revision,
          });
          return;
        }
        case "open_url": {
          const url = action.action.url.trim();
          if (/^https?:\/\//i.test(url)) {
            await openUrl(url);
            return;
          }
          if (demo) {
            onError("Workspace links are not available in browser demo mode.");
            return;
          }
          await emitTo("main", "open-resource", { root, path: url });
          return;
        }
        default: {
          const _exhaustive: never = action.action;
          return _exhaustive;
        }
      }
    },
    [
      activeView,
      busy,
      columns,
      demo,
      onError,
      onOpenForm,
      onSnapshot,
      packageRevision,
      readOnly,
      relPath,
      root,
      row,
      rowFetchLimit,
    ],
  );

  const handleRun = useCallback(
    (action: ActionSummary) => {
      void runAction(action).catch((err) => {
        const message = String(err);
        if (isStaleRevisionError(message)) {
          onStale();
          onError("This table changed elsewhere. Reload before running actions.");
          return;
        }
        onError(message);
      });
    },
    [onError, onStale, runAction],
  );

  if (visibleActions.length === 0 && !loadError) {
    return null;
  }

  return (
    <div className="data-table-action-menu">
      <button
        type="button"
        className="secondary-button data-table-action-trigger"
        disabled={busy || (scope === "row" && !row)}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
      >
        {menuLabel}
      </button>
      {open && (
        <div className="data-table-action-popover" role="menu">
          {loadError && <p className="data-table-action-error">{loadError}</p>}
          {visibleActions.map((action) => (
            <button
              key={action.name}
              type="button"
              role="menuitem"
              className="data-table-action-item"
              disabled={busy || readOnly}
              onClick={() => handleRun(action)}
            >
              {action.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
