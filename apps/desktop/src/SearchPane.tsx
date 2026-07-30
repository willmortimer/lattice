import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { invoke } from "./lib/ipc";
import {
  enableSemanticSearch,
  getSemanticStatus,
  listenSemanticEvents,
  type SemanticStatus,
} from "./lib/semantic";
import {
  formatSearchHitScore,
  looksFtsOnlyWhileSemanticEnabled,
  SEARCH_VECTORS_BEHIND_BANNER,
  searchHitBadgeKind,
  searchHitBadgeLabel,
  shouldShowVectorsBehindBanner,
} from "./lib/searchHitBadge";

import { KindMark } from "./KindMark";
import type { SearchHit } from "./types";

const SEARCH_DEBOUNCE_MS = 150;
const SEARCH_LIMIT = 30;

interface SearchPaneProps {
  /** `null` in the in-browser demo shell, or before a workspace is open. */
  root: string | null;
  /** When true, native search uses hybrid `auto` mode; demo search is unchanged. */
  semanticEnabled?: boolean;
  /** Stand-in for `search_workspace` when `root` is `null`. */
  demoSearch: (query: string) => SearchHit[];
  onOpenFile: (path: string) => void;
  onClose: () => void;
}

function searchHitKey(hit: SearchHit, index: number): string {
  return hit.chunkId ? `${hit.path}:${hit.chunkId}` : `${hit.path}:${index}`;
}

/**
 * Cmd/Ctrl+K search pane over `search_workspace` (docs/21).
 * When `semanticEnabled`, uses mode `auto` (hybrid when the semantic worker is
 * ready; otherwise FTS). Hits use a `page` kind mark for now.
 */
export function SearchPane({
  root,
  semanticEnabled = false,
  demoSearch,
  onOpenFile,
  onClose,
}: SearchPaneProps) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [semanticStatus, setSemanticStatus] = useState<SemanticStatus | null>(null);
  const [refreshingVectors, setRefreshingVectors] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    if (!root || !semanticEnabled) {
      setSemanticStatus(null);
      return;
    }

    let cancelled = false;
    void getSemanticStatus(root)
      .then((status) => {
        if (!cancelled) setSemanticStatus(status);
      })
      .catch(() => {
        /* keep last status */
      });

    let unlisten: (() => void) | undefined;
    void listenSemanticEvents((event) => {
      if (event.type !== "status") return;
      setSemanticStatus({
        state: event.state,
        pendingChunks: event.pendingChunks,
        message: event.message,
        progressPercent: event.progressPercent,
        providerId: event.providerId,
        modelId: event.modelId,
        dimensions: event.dimensions,
      });
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [root, semanticEnabled]);

  useEffect(() => {
    if (!root || !semanticEnabled) return;
    if (
      !semanticStatus ||
      (semanticStatus.state !== "downloading" &&
        semanticStatus.state !== "preparing" &&
        semanticStatus.state !== "indexing")
    ) {
      return;
    }

    const id = window.setInterval(() => {
      void getSemanticStatus(root)
        .then((next) => setSemanticStatus(next))
        .catch(() => {
          /* keep last status */
        });
    }, 750);

    return () => window.clearInterval(id);
  }, [root, semanticEnabled, semanticStatus?.state]);

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
          ...(semanticEnabled ? { mode: "auto" as const } : {}),
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
  }, [query, root, demoSearch, semanticEnabled]);

  const showIndexingHint =
    Boolean(root) && looksFtsOnlyWhileSemanticEnabled(semanticEnabled, hits);
  const showVectorsBehindBanner = shouldShowVectorsBehindBanner(semanticEnabled, semanticStatus);

  function handleRefreshVectors() {
    if (!root || refreshingVectors) return;
    setRefreshingVectors(true);
    void enableSemanticSearch(root)
      .then((next) => setSemanticStatus(next))
      .finally(() => setRefreshingVectors(false));
  }

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
        {showVectorsBehindBanner && (
          <p className="search-pane-status-hint" role="status">
            {SEARCH_VECTORS_BEHIND_BANNER}{" "}
            <button
              type="button"
              className="search-pane-status-action"
              disabled={refreshingVectors}
              onClick={handleRefreshVectors}
            >
              {refreshingVectors ? "Refreshing…" : "Refresh vectors"}
            </button>
          </p>
        )}
        {showIndexingHint && !showVectorsBehindBanner && (
          <p className="search-pane-status-hint" role="status">
            Semantic index still preparing — keyword matches only for now.
          </p>
        )}
        <div className="palette-list">
          {error && <p className="error-text search-pane-error">{error}</p>}
          {!error && query.trim().length > 0 && hits.length === 0 && (
            <div className="palette-empty">No matches.</div>
          )}
          {hits.map((hit, index) => {
            const badgeKind = searchHitBadgeKind(hit);
            const scoreLabel = formatSearchHitScore(hit);
            return (
              <button
                key={searchHitKey(hit, index)}
                className="palette-item search-hit"
                onClick={() => onOpenFile(hit.path)}
              >
                <KindMark kind="page" />
                <span className="search-hit-body">
                  <span className="search-hit-title-row">
                    <span className="palette-item-label">{hit.title || hit.path}</span>
                    {(badgeKind || scoreLabel) && (
                      <span className="search-hit-match-meta">
                        {badgeKind && (
                          <span
                            className="search-hit-badge"
                            aria-label={`${searchHitBadgeLabel(badgeKind)} match`}
                          >
                            {searchHitBadgeLabel(badgeKind)}
                          </span>
                        )}
                        {scoreLabel && (
                          <span className="search-hit-score" aria-label={`Fusion ${scoreLabel}`}>
                            {scoreLabel}
                          </span>
                        )}
                      </span>
                    )}
                  </span>
                  {hit.snippet && <span className="search-hit-snippet">{hit.snippet}</span>}
                </span>
                <span className="palette-item-hint">{hit.path}</span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
