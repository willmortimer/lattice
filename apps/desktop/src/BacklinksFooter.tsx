import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { Backlink } from "./types";

interface BacklinksFooterProps {
  /** `null` in the in-browser demo shell, or before a workspace is open. */
  root: string | null;
  path: string;
  onOpenFile: (path: string) => void;
  /** Stand-in for `get_backlinks` when `root` is `null`. */
  demoBacklinks?: Backlink[];
}

/**
 * "Linked from" footer under an open page (WS4's backlinks query,
 * surfaced in the shell per docs/21's stable-links model). Renders
 * nothing when there are no backlinks, rather than an empty section.
 */
export function BacklinksFooter({ root, path, onOpenFile, demoBacklinks }: BacklinksFooterProps) {
  const [backlinks, setBacklinks] = useState<Backlink[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setError(null);

    if (!root) {
      setBacklinks(demoBacklinks ?? []);
      return;
    }

    invoke<Backlink[]>("get_backlinks", { root, relPath: path })
      .then((result) => {
        if (!cancelled) setBacklinks(result);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(String(err));
      });

    return () => {
      cancelled = true;
    };
  }, [root, path, demoBacklinks]);

  if (error) {
    return <p className="error-text backlinks-error">{error}</p>;
  }
  if (backlinks.length === 0) {
    return null;
  }

  return (
    <footer className="backlinks-footer">
      <h2 className="backlinks-title">Linked from</h2>
      <ul className="backlinks-list">
        {backlinks.map((link) => (
          <li key={`${link.source_path}:${link.target}:${link.anchor ?? ""}`}>
            <button className="backlinks-item" onClick={() => onOpenFile(link.source_path)}>
              {link.source_path}
            </button>
          </li>
        ))}
      </ul>
    </footer>
  );
}
