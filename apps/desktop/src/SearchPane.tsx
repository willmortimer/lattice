import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";

import { KindMark } from "./KindMark";
import type { SearchHit } from "./types";

const SEARCH_DEBOUNCE_MS = 150;
const SEARCH_LIMIT = 30;

interface SearchPaneProps {
  /** `null` in the in-browser demo shell, or before a workspace is open. */
  root: string | null;
  /** Stand-in for `search_workspace` when `root` is `null`. */
  demoSearch: (query: string) => SearchHit[];
  onOpenFile: (path: string) => void;
  onClose: () => void;
}

/**
 * Cmd/Ctrl+Shift+F search pane over `search_workspace` (WS4's FTS index,
 * surfaced in the shell per docs/21's search stack). All hits are pages —
 * the index only covers Markdown — so every result gets a `page` kind mark.
 */
export function SearchPane({ root, demoSearch, onOpenFile, onClose }: SearchPaneProps) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const trimmed = query.trim();
    if (trimmed.length === 0) {
      setHits([]);
      setError(null);
      return;
    }

    async function runSearch(text: string): Promise<{ hits: SearchHit[] } | { error: string }> {
      if (!root) {
        return { hits: demoSearch(text) };
      }
      try {
        const results = await invoke<SearchHit[]>("search_workspace", {
          root,
          query: text,
          limit: SEARCH_LIMIT,
        });
        return { hits: results };
      } catch (err) {
        return { error: String(err) };
      }
    }

    let cancelled = false;
    const timer = window.setTimeout(() => {
      runSearch(trimmed).then((result) => {
        if (cancelled) return;
        if ("hits" in result) {
          setHits(result.hits);
          setError(null);
        } else {
          setError(result.error);
        }
      });
    }, SEARCH_DEBOUNCE_MS);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [query, root, demoSearch]);

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    }
  }

  return (
    <div className="palette-overlay" onMouseDown={onClose}>
      <div
        className="palette search-pane"
        role="dialog"
        aria-label="Search workspace"
        onMouseDown={(event) => event.stopPropagation()}
        onKeyDown={onKeyDown}
      >
        <input
          ref={inputRef}
          className="palette-input"
          placeholder="Search pages…"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          aria-label="Search query"
        />
        <div className="palette-list">
          {error && <p className="error-text search-pane-error">{error}</p>}
          {!error && query.trim().length > 0 && hits.length === 0 && (
            <div className="palette-empty">No matches.</div>
          )}
          {hits.map((hit) => (
            <button
              key={hit.path}
              className="palette-item search-hit"
              onClick={() => onOpenFile(hit.path)}
            >
              <KindMark kind="page" />
              <span className="search-hit-body">
                <span className="palette-item-label">{hit.title || hit.path}</span>
                {hit.snippet && <span className="search-hit-snippet">{hit.snippet}</span>}
              </span>
              <span className="palette-item-hint">{hit.path}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
