import { IconButton } from "@lattice/ui";
import { invoke } from "@tauri-apps/api/core";
import { X } from "lucide-react";
import { useEffect, useState } from "react";

import type { DataAppSnapshot } from "../data/types";
import { inBrowser } from "../demo";
import { KIND_LABELS } from "../KindMark";
import type { Backlink, Resource } from "../types";
import { InspectorHistoryPanel } from "./InspectorHistoryPanel";

const SECTIONS = [
  "properties",
  "links",
  "history",
  "schema",
  "source",
  "permissions",
  "diagnostics",
] as const;

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
  const [loading, setLoading] = useState(false);

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
    void Promise.all(tasks)
      .catch(() => {
        if (!cancelled) {
          if (section === "history") setHistory([]);
          if (section === "links") setBacklinks([]);
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [resource, root, section]);

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
          <dl className="property-list">
            <div><dt>Kind</dt><dd>{resource ? KIND_LABELS[resource.kind] : "Workspace"}</dd></div>
            <div><dt>Path</dt><dd>{resource?.path ?? "—"}</dd></div>
            <div><dt>Format</dt><dd>{resource?.formatId ?? "—"}</dd></div>
            <div><dt>Canonical state</dt><dd>{resource ? "Workspace file" : "Directory"}</dd></div>
          </dl>
        )}
        {!loading && section === "links" && (
          <>
            {resource?.kind !== "page" && <p className="inspector-empty">Links are available for pages.</p>}
            {resource?.kind === "page" && backlinks.length === 0 && <p className="inspector-empty">No indexed backlinks.</p>}
            {backlinks.map((link) => (
              <button type="button" className="inspector-link" key={`${link.source_path}:${link.target}`} onClick={() => onOpenFile(link.source_path)}>
                {link.source_path}
              </button>
            ))}
          </>
        )}
        {section === "history" && root && resource && !inBrowser && (
          <InspectorHistoryPanel root={root} path={resource.path} />
        )}
        {!loading && section === "history" && !(root && resource && !inBrowser) && (
          <div className="history-list">
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
