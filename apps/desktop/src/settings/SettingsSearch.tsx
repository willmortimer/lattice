import { MagnifyingGlass } from "@phosphor-icons/react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";

import {
  filterSettingsSearch,
  sectionLabel,
  type SettingsSearchItem,
} from "./settingsCatalog";

const ESTIMATED_RESULT_HEIGHT = 52;
const RESULT_OVERSCAN = 4;

interface SettingsSearchProps {
  onSelect: (item: SettingsSearchItem) => void;
}

export function SettingsSearch({ onSelect }: SettingsSearchProps) {
  const listboxId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const results = filterSettingsSearch(query);

  const virtualizer = useVirtualizer({
    count: results.length,
    estimateSize: () => ESTIMATED_RESULT_HEIGHT,
    getItemKey: (index) => results[index]!.id,
    getScrollElement: () => listRef.current,
    overscan: RESULT_OVERSCAN,
  });

  useEffect(() => {
    setActiveIndex(0);
  }, [query]);

  useLayoutEffect(() => {
    if (!open || results.length === 0) {
      return;
    }
    virtualizer.scrollToIndex(activeIndex, { align: "auto" });
  }, [activeIndex, open, results.length, virtualizer]);

  function selectItem(item: SettingsSearchItem) {
    onSelect(item);
    setQuery("");
    setOpen(false);
    inputRef.current?.blur();
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if (!open || results.length === 0) {
      if (event.key === "Escape") {
        setQuery("");
        setOpen(false);
      }
      return;
    }

    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        setActiveIndex((index) => (index + 1) % results.length);
        break;
      case "ArrowUp":
        event.preventDefault();
        setActiveIndex((index) => (index - 1 + results.length) % results.length);
        break;
      case "Enter":
        event.preventDefault();
        selectItem(results[activeIndex] ?? results[0]);
        break;
      case "Escape":
        event.preventDefault();
        setQuery("");
        setOpen(false);
        break;
      default:
        break;
    }
  }

  const showResults = open && query.trim() && results.length > 0;
  const activeOptionId = showResults ? `${listboxId}-option-${activeIndex}` : undefined;
  const virtualItems = virtualizer.getVirtualItems();
  const paddingTop = virtualItems[0]?.start ?? 0;
  const paddingBottom = Math.max(
    0,
    virtualizer.getTotalSize() - (virtualItems.at(-1)?.end ?? 0),
  );

  return (
    <div className="settings-search">
      <MagnifyingGlass size={14} aria-hidden="true" className="settings-search-icon" />
      <input
        ref={inputRef}
        type="search"
        role="combobox"
        aria-expanded={showResults ? true : false}
        aria-controls={showResults ? listboxId : undefined}
        aria-autocomplete="list"
        aria-activedescendant={activeOptionId}
        placeholder="Search settings"
        className="settings-search-input"
        value={query}
        onChange={(event) => {
          setQuery(event.currentTarget.value);
          setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        onBlur={() => {
          window.setTimeout(() => setOpen(false), 120);
        }}
        onKeyDown={handleKeyDown}
      />
      {showResults ? (
        <ul
          ref={listRef}
          id={listboxId}
          role="listbox"
          className="settings-search-results"
        >
          <div style={{ paddingTop, paddingBottom }}>
            {virtualItems.map((virtualRow) => {
              const index = virtualRow.index;
              const item = results[index]!;
              const optionId = `${listboxId}-option-${index}`;
              return (
                <li
                  key={virtualRow.key}
                  role="presentation"
                  data-index={index}
                  ref={virtualizer.measureElement}
                >
                  <button
                    id={optionId}
                    type="button"
                    role="option"
                    aria-selected={index === activeIndex}
                    className={index === activeIndex ? "settings-search-result-active" : ""}
                    onMouseDown={(event) => event.preventDefault()}
                    onMouseEnter={() => setActiveIndex(index)}
                    onClick={() => selectItem(item)}
                  >
                    <strong>{item.title}</strong>
                    <span>
                      {sectionLabel(item.section)} · {item.description}
                    </span>
                  </button>
                </li>
              );
            })}
          </div>
        </ul>
      ) : null}
      {open && query.trim() && results.length === 0 ? (
        <p className="settings-search-empty" role="status">
          No matching settings
        </p>
      ) : null}
    </div>
  );
}
