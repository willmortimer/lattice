import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { invoke } from "./lib/ipc";
import {
  enableSemanticSearch,
} from "./lib/semantic";
import {
  setSemanticStatusCache,
  useSemanticStatusQuery,
} from "./query/useSemanticStatusQuery";
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
const ESTIMATED_HIT_HEIGHT = 72;
const HIT_OVERSCAN = 4;

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
  const listboxId = useId();
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [refreshingVectors, setRefreshingVectors] = useState(false);
  const [highlighted, setHighlighted] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const queryClient = useQueryClient();
  const { data: semanticStatus = null } = useSemanticStatusQuery(
    root && semanticEnabled ? root : null,
  );

  const virtualizer = useVirtualizer({
    count: hits.length,
    estimateSize: () => ESTIMATED_HIT_HEIGHT,
    getItemKey: (index) => searchHitKey(hits[index]!, index),
    getScrollElement: () => listRef.current,
    overscan: HIT_OVERSCAN,
  });

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    setHighlighted(0);
  }, [query, hits]);

  useLayoutEffect(() => {
    if (hits.length === 0) {
      return;
    }
    virtualizer.scrollToIndex(highlighted, { align: "auto" });
  }, [highlighted, hits.length, virtualizer]);

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
  const trimmedQuery = query.trim();
  const showHits = !error && trimmedQuery.length > 0 && hits.length > 0;
  const activeOptionId = showHits ? `${listboxId}-option-${highlighted}` : undefined;

  function openHit(hit: SearchHit | undefined) {
    if (!hit) return;
    onOpenFile(hit.path);
    onClose();
  }

  function handleRefreshVectors() {
    if (!root || refreshingVectors) return;
    setRefreshingVectors(true);
    void enableSemanticSearch(root)
      .then((next) => setSemanticStatusCache(queryClient, root, next))
      .finally(() => setRefreshingVectors(false));
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
      return;
    }

    if (!showHits) {
      return;
    }

    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        setHighlighted((index) => Math.min(index + 1, hits.length - 1));
        break;
      case "ArrowUp":
        event.preventDefault();
        setHighlighted((index) => Math.max(index - 1, 0));
        break;
      case "Enter":
        event.preventDefault();
        openHit(hits[highlighted]);
        break;
      default:
        break;
    }
  }

  const virtualItems = virtualizer.getVirtualItems();
  const paddingTop = virtualItems[0]?.start ?? 0;
  const paddingBottom = Math.max(
    0,
    virtualizer.getTotalSize() - (virtualItems.at(-1)?.end ?? 0),
  );

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
          role="combobox"
          aria-expanded={showHits ? true : false}
          aria-controls={showHits ? listboxId : undefined}
          aria-autocomplete="list"
          aria-activedescendant={activeOptionId}
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
        <div
          ref={listRef}
          id={listboxId}
          className="palette-list"
          role="listbox"
        >
          {error && <p className="error-text search-pane-error">{error}</p>}
          {!error && trimmedQuery.length > 0 && hits.length === 0 && (
            <div className="palette-empty">No matches.</div>
          )}
          {showHits ? (
            <div style={{ paddingTop, paddingBottom }}>
              {virtualItems.map((virtualRow) => {
                const index = virtualRow.index;
                const hit = hits[index]!;
                const badgeKind = searchHitBadgeKind(hit);
                const scoreLabel = formatSearchHitScore(hit);
                const optionId = `${listboxId}-option-${index}`;
                return (
                  <button
                    key={virtualRow.key}
                    id={optionId}
                    data-index={index}
                    ref={virtualizer.measureElement}
                    className={
                      "palette-item search-hit"
                      + (index === highlighted ? " palette-item-active" : "")
                    }
                    role="option"
                    aria-selected={index === highlighted}
                    onMouseEnter={() => setHighlighted(index)}
                    onClick={() => openHit(hit)}
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
          ) : null}
        </div>
      </div>
    </div>
  );
}
