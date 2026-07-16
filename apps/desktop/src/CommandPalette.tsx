import { useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent } from "react";

import { KindMark } from "./KindMark";
import { fuzzyFilter } from "./lib/fuzzy";
import type { ResourceKind } from "./types";

/** One entry in the palette: a workspace file (`kind` set) or a shell action. */
export interface PaletteItem {
  id: string;
  label: string;
  /** Shown faded to the right — a file's path, or an action's shortcut. */
  hint?: string;
  kind?: ResourceKind;
  run: () => void;
}

interface CommandPaletteProps {
  items: PaletteItem[];
  onClose: () => void;
}

/**
 * Cmd/Ctrl+P command palette: fuzzy-filters `items` (workspace files plus
 * shell actions) by label and hint. Enter runs the highlighted item and
 * closes the palette; Escape closes without running anything.
 */
export function CommandPalette({ items, onClose }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [highlighted, setHighlighted] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const matches = useMemo(
    () => fuzzyFilter(items, query, (item) => `${item.label} ${item.hint ?? ""}`).map((r) => r.item),
    [items, query],
  );

  useEffect(() => {
    setHighlighted(0);
  }, [query]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  function runItem(item: PaletteItem | undefined) {
    if (!item) return;
    onClose();
    item.run();
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      setHighlighted((i) => Math.min(i + 1, matches.length - 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setHighlighted((i) => Math.max(i - 1, 0));
    } else if (event.key === "Enter") {
      event.preventDefault();
      runItem(matches[highlighted]);
    }
  }

  return (
    <div className="palette-overlay" onMouseDown={onClose}>
      <div
        className="palette"
        role="dialog"
        aria-label="Command palette"
        onMouseDown={(event) => event.stopPropagation()}
        onKeyDown={onKeyDown}
      >
        <input
          ref={inputRef}
          className="palette-input"
          placeholder="Jump to a file, or run a command…"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          aria-label="Command palette query"
        />
        <div className="palette-list" role="listbox">
          {matches.length === 0 && <div className="palette-empty">No matches.</div>}
          {matches.map((item, index) => (
            <button
              key={item.id}
              className={"palette-item" + (index === highlighted ? " palette-item-active" : "")}
              onMouseEnter={() => setHighlighted(index)}
              onClick={() => runItem(item)}
              role="option"
              aria-selected={index === highlighted}
            >
              {item.kind && <KindMark kind={item.kind} />}
              <span className="palette-item-label">{item.label}</span>
              {item.hint && <span className="palette-item-hint">{item.hint}</span>}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
