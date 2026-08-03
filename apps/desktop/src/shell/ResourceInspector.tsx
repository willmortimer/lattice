import { Button, IconButton } from "@lattice/ui";
import { invoke } from "../lib/ipc";
import { cloudBlobOpen } from "../lib/cloud";
import {
  backupResourceToCloud,
  isCloudBackupResource,
  openCloudAccountSettings,
} from "../lib/cloudBackup";
import {
  displayResourceIdForPath,
} from "../lib/resourceCatalog";
import {
  formatAuthority,
  formatMaterialization,
  getResourceStat,
  type ResourceStat,
} from "../lib/resourceStat";
import {
  listRelationshipEdges,
  RELATIONSHIP_MODE_PRESETS,
  type RelationshipEdge,
  type RelationshipMode,
} from "../lib/relationshipGraph";
import { X } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import type { DataAppSnapshot } from "../data/types";
import { inBrowser } from "../demo";
import { KIND_LABELS } from "../KindMark";
import type { Backlink, Resource } from "../types";
import { InspectorHistoryPanel } from "./InspectorHistoryPanel";
import { useDesktopUiStore } from "./desktopUiStore";

const SECTIONS = [
  "properties",
  "links",
  "graph",
  "history",
  "schema",
  "source",
  "permissions",
  "diagnostics",
] as const;

const GRAPH_MODES: { id: RelationshipMode; label: string }[] = [
  { id: "all", label: "All" },
  { id: "knowledge", label: "Knowledge" },
  { id: "data", label: "Data" },
  { id: "execution", label: "Execution" },
];

interface HistoryItem {
  id: string;
  summary: string;
  createdAt: number;
  undone: boolean;
  commandCount: number;
}

function fileTitle(path: string): string {
  const base = path.split("/").pop() ?? path;
  return base.replace(/\.(md|canvas|pdf|png|jpe?g)$/i, "").replace(/\.data$/i, "");
}

function otherEnd(edge: RelationshipEdge, focus: string): string {
  const focusStem = focus.replace(/\.md$/i, "");
  const fromStem = edge.from.replace(/\.md$/i, "");
  if (edge.from === focus || fromStem === focusStem || edge.from.startsWith(`${focus}#`)) {
    return edge.to;
  }
  return edge.from;
}

export function ResourceInspector({
  root,
  resource,
  pageContent,
  dataSnapshot,
  error,
  onClose,
  onOpenFile,
}: {
  root: string | null;
  resource: Resource | null;
  pageContent: string | null;
  dataSnapshot: DataAppSnapshot | null;
  error: string | null;
  onClose: () => void;
  onOpenFile: (path: string) => void;
}) {
  const [section, setSection] = useState<(typeof SECTIONS)[number]>("properties");
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [backlinks, setBacklinks] = useState<Backlink[]>([]);
  const [graphEdges, setGraphEdges] = useState<RelationshipEdge[]>([]);
  const [graphMode, setGraphMode] = useState<RelationshipMode>("all");
  const [resourceStat, setResourceStat] = useState<ResourceStat | null>(null);
  const [loading, setLoading] = useState(false);
  const [cloudBusy, setCloudBusy] = useState(false);
  const [cloudError, setCloudError] = useState<string | null>(null);
  const [cloudOpenBytes, setCloudOpenBytes] = useState<number | null>(null);
  const recordAuthorityStat = useDesktopUiStore((state) => state.recordAuthorityStat);

  useEffect(() => {
    setResourceStat(null);
    setCloudError(null);
    setCloudOpenBytes(null);
  }, [resource?.path, root]);

  useEffect(() => {
    if (!root || inBrowser) return;
    // Per-resource history is owned by InspectorHistoryPanel.
    if (section === "history" && resource) {
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    const tasks: Promise<void>[] = [];
    if (section === "properties" && resource) {
      tasks.push(
        getResourceStat(root, resource.path).then((stat) => {
          if (!cancelled) {
            setResourceStat(stat);
            recordAuthorityStat(stat);
          }
        }),
      );
    }
    if (section === "history" && !resource) {
      tasks.push(
        invoke<HistoryItem[]>("list_history", { root, limit: 30 }).then((items) => {
          if (!cancelled) setHistory(items);
        }),
      );
    }
    if (section === "links" && resource?.kind === "page") {
      tasks.push(
        invoke<Backlink[]>("get_backlinks", { root, relPath: resource.path }).then((items) => {
          if (!cancelled) setBacklinks(items);
        }),
      );
    }
    if (section === "graph" && resource) {
      const kinds = RELATIONSHIP_MODE_PRESETS[graphMode];
      tasks.push(
        listRelationshipEdges({
          root,
          focusPath: resource.path,
          kinds,
        }).then((edges) => {
          if (!cancelled) setGraphEdges(edges);
        }),
      );
    }
    void Promise.all(tasks)
      .catch(() => {
        if (!cancelled) {
          if (section === "properties") setResourceStat(null);
          if (section === "history") setHistory([]);
          if (section === "links") setBacklinks([]);
          if (section === "graph") setGraphEdges([]);
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [resource, root, section, graphMode, recordAuthorityStat]);

  const displayId = resource
    ? displayResourceIdForPath(resource.path, resourceStat?.resource_id)
    : null;

  async function handleCloudBackup() {
    if (inBrowser || cloudBusy || !root || !resource) return;
    setCloudBusy(true);
    setCloudError(null);
    setCloudOpenBytes(null);
    try {
      const result = await backupResourceToCloud(root, resource.path);
      if (!result.ok) {
        if (result.reason === "signed_out") {
          openCloudAccountSettings();
        }
        setCloudError(result.message);
        return;
      }
      setResourceStat(result.stat);
      recordAuthorityStat(result.stat);
    } finally {
      setCloudBusy(false);
    }
  }

  async function handleCloudReopen() {
    if (inBrowser || cloudBusy || !root || !resource) return;
    setCloudBusy(true);
    setCloudError(null);
    try {
      const bytes = await cloudBlobOpen(root, resource.path);
      setCloudOpenBytes(bytes.length);
      const stat = await getResourceStat(root, resource.path);
      setResourceStat(stat);
      recordAuthorityStat(stat);
    } catch (err: unknown) {
      setCloudOpenBytes(null);
      setCloudError(err instanceof Error ? err.message : String(err));
    } finally {
      setCloudBusy(false);
    }
  }

  const canCloudBackup = Boolean(
    root && resource && isCloudBackupResource(resource.kind) && !inBrowser,
  );
  const isCloudAuthority = resourceStat?.authority === "cloud";

  return (
    <aside className="inspector">
      <header className="inspector-head">
        <div>
          <span className="inspector-eyebrow">Inspect</span>
          <strong>{resource ? fileTitle(resource.path) : "Workspace"}</strong>
        </div>
        <IconButton label="Close inspector" onClick={onClose}>
          <X size={15} />
        </IconButton>
      </header>
      <nav className="inspector-sections" aria-label="Inspector sections">
        {SECTIONS.map((name) => (
          <button
            type="button"
            key={name}
            className={section === name ? "inspector-section-active" : ""}
            onClick={() => setSection(name)}
          >
            {name}
          </button>
        ))}
      </nav>
      <div className="inspector-body">
        {loading && <p className="inspector-empty">Loading…</p>}
        {!loading && section === "properties" && (
          <>
            <dl className="property-list">
              <div><dt>Kind</dt><dd>{resource ? KIND_LABELS[resource.kind] : "Workspace"}</dd></div>
              <div><dt>Path</dt><dd>{resource?.path ?? "—"}</dd></div>
              <div><dt>Format</dt><dd>{resource?.formatId ?? "—"}</dd></div>
              <div><dt>Canonical state</dt><dd>{resource ? "Workspace file" : "Directory"}</dd></div>
              {resource && displayId && (
                <div>
                  <dt>Resource ID</dt>
                  <dd>
                    <code>{displayId.resourceId}</code>
                    {displayId.isSynthetic ? (
                      <span className="inspector-id-note"> placeholder until registry assigns one</span>
                    ) : null}
                  </dd>
                </div>
              )}
              {resource && resourceStat && (
                <>
                  <div><dt>Authority</dt><dd>{formatAuthority(resourceStat.authority)}</dd></div>
                  <div><dt>Materialization</dt><dd>{formatMaterialization(resourceStat.materialization)}</dd></div>
                  {resourceStat.content_hash && (
                    <div><dt>Content hash</dt><dd><code>{resourceStat.content_hash}</code></dd></div>
                  )}
                  {resourceStat.version_id && (
                    <div><dt>Version ID</dt><dd><code>{resourceStat.version_id}</code></dd></div>
                  )}
                  {resourceStat.hydration_inputs && resourceStat.hydration_inputs.length > 0 && (
                    <div>
                      <dt>Hydration inputs</dt>
                      <dd>
                        <ul className="inspector-hydration-list">
                          {resourceStat.hydration_inputs.map((digest) => (
                            <li key={`${digest.path}:${digest.contentHash}:${digest.resourceId ?? ""}`}>
                              <code>{digest.path}</code>
                              {" @ "}
                              <code>{digest.contentHash}</code>
                              {digest.resourceId ? (
                                <>
                                  {" "}
                                  (<code>{digest.resourceId}</code>)
                                </>
                              ) : null}
                            </li>
                          ))}
                        </ul>
                      </dd>
                    </div>
                  )}
                </>
              )}
              {resource && !resourceStat && inBrowser && (
                <div>
                  <dt>Authority</dt>
                  <dd>Local (browser demo)</dd>
                </div>
              )}
            </dl>
            {canCloudBackup && (
              <div className="inspector-cloud-actions">
                <p className="inspector-cloud-copy">
                  Back up this resource to Lattice Cloud. Requires Settings → Cloud account.
                  Authority becomes Cloud after a successful upload. You can also back up from the
                  Files tree context menu or command palette.
                </p>
                <div className="cloud-account-actions">
                  <Button
                    size="sm"
                    disabled={cloudBusy}
                    onClick={() => void handleCloudBackup()}
                  >
                    {cloudBusy ? "Working…" : "Back up to Lattice Cloud"}
                  </Button>
                  <Button
                    size="sm"
                    variant="secondary"
                    disabled={cloudBusy || !isCloudAuthority}
                    onClick={() => void handleCloudReopen()}
                  >
                    {cloudBusy ? "Working…" : "Reopen from cloud"}
                  </Button>
                </div>
                {isCloudAuthority ? (
                  <p className="inspector-cloud-status" role="status">
                    Authority {formatAuthority("cloud")}
                    {resourceStat?.content_hash ? ` · ${resourceStat.content_hash}` : ""}
                  </p>
                ) : null}
                {cloudOpenBytes !== null ? (
                  <p className="inspector-cloud-status" role="status">
                    Reopened from cloud · {cloudOpenBytes} bytes
                  </p>
                ) : null}
                {cloudError ? (
                  <p className="inspector-cloud-error" role="alert">
                    {cloudError}
                  </p>
                ) : null}
              </div>
            )}
          </>
        )}
        {!loading && section === "links" && (
          <>
            {resource?.kind !== "page" && <p className="inspector-empty">Links are available for pages.</p>}
            {resource?.kind === "page" && backlinks.length === 0 && <p className="inspector-empty">No indexed backlinks.</p>}
            {backlinks.map((link, index) => (
              <button
                type="button"
                className="inspector-link"
                key={`${link.source_path}:${link.target}:${link.anchor ?? ""}:${index}`}
                onClick={() => onOpenFile(link.source_path)}
              >
                {link.source_path}
              </button>
            ))}
          </>
        )}
        {!loading && section === "graph" && (
          <div className="inspector-graph">
            {!resource && <p className="inspector-empty">Select a resource to inspect its neighborhood.</p>}
            {resource && (
              <>
                <p className="inspector-graph-focus">
                  Focus <code>{resource.path}</code>
                </p>
                <div className="inspector-graph-modes" role="group" aria-label="Relationship modes">
                  {GRAPH_MODES.map((mode) => (
                    <button
                      type="button"
                      key={mode.id}
                      className={graphMode === mode.id ? "inspector-section-active" : ""}
                      onClick={() => setGraphMode(mode.id)}
                    >
                      {mode.label}
                    </button>
                  ))}
                </div>
                {graphEdges.length === 0 && (
                  <p className="inspector-empty">
                    No relationship edges for this mode
                    {graphMode === "all"
                      ? " (semantic similarity is not implemented yet)."
                      : "."}
                  </p>
                )}
                <ul className="inspector-graph-list">
                  {graphEdges.map((edge, index) => {
                    const neighbor = otherEnd(edge, resource.path);
                    const openPath = neighbor.includes("#")
                      ? neighbor.slice(0, neighbor.indexOf("#"))
                      : neighbor;
                    return (
                      <li key={`${edge.kind}:${edge.from}:${edge.to}:${index}`}>
                        <button
                          type="button"
                          className="inspector-graph-edge"
                          onClick={() => onOpenFile(openPath)}
                        >
                          <span className="inspector-graph-kind">{edge.kind}</span>
                          <span className="inspector-graph-dir" aria-hidden="true">
                            {edge.from === resource.path ||
                            edge.from.replace(/\.md$/i, "") === resource.path.replace(/\.md$/i, "") ||
                            edge.from.startsWith(`${resource.path}#`)
                              ? "→"
                              : "←"}
                          </span>
                          <span className="inspector-graph-neighbor">{neighbor}</span>
                        </button>
                      </li>
                    );
                  })}
                </ul>
              </>
            )}
          </div>
        )}
        {section === "history" && root && resource && !inBrowser && (
          <InspectorHistoryPanel root={root} path={resource.path} />
        )}
        {!loading && section === "history" && !(root && resource && !inBrowser) && (
          <div className="history-list">
            <p className="inspector-empty">
              Path changes that include link repair may appear as rename-shaped history entries.
            </p>
            {history.length === 0 && <p className="inspector-empty">No command history yet.</p>}
            {history.map((item) => (
              <article key={item.id}>
                <strong>{item.summary}</strong>
                <span>{new Date(item.createdAt * 1000).toLocaleString()} · {item.commandCount} command{item.commandCount === 1 ? "" : "s"}{item.undone ? " · undone" : ""}</span>
              </article>
            ))}
          </div>
        )}
        {!loading && section === "schema" && (
          <>
            {!dataSnapshot && <p className="inspector-empty">Open a table to inspect its schema.</p>}
            {dataSnapshot?.columns.map((column) => (
              <div className="schema-row" key={column.name}><strong>{column.name}</strong><span>{column.field_type}</span></div>
            ))}
          </>
        )}
        {!loading && section === "source" && (
          <pre className="inspector-source">{pageContent ?? (dataSnapshot ? JSON.stringify(dataSnapshot, null, 2) : resource?.path ?? "No source")}</pre>
        )}
        {!loading && section === "permissions" && (
          <div className="inspector-copy"><p>Local workspace access</p><span>Reads are scoped to this directory. Mutations are validated and recorded by the semantic command core.</span></div>
        )}
        {!loading && section === "diagnostics" && (
          <div className="inspector-copy"><p>{error ? "Problem reported" : "No active diagnostics"}</p><span>{error ?? "The selected resource is loaded without a reported conflict."}</span></div>
        )}
      </div>
    </aside>
  );
}
