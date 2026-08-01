import { MagnifyingGlass } from "@phosphor-icons/react";
import { useEffect, useId, useRef, useState } from "react";

import {
  filterSettingsSearch,
  sectionLabel,
  type SettingsSearchItem,
} from "./settingsCatalog";

interface SettingsSearchProps {
  onSelect: (item: SettingsSearchItem) => void;
}

export function SettingsSearch({ onSelect }: SettingsSearchProps) {
  const listboxId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const results = filterSettingsSearch(query);

  useEffect(() => {
    setActiveIndex(0);
  }, [query]);

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

  return (
    <div className="settings-search">
      <MagnifyingGlass size={14} aria-hidden="true" className="settings-search-icon" />
      <input
        ref={inputRef}
        type="search"
        role="combobox"
        aria-expanded={open && results.length > 0}
        aria-controls={listboxId}
        aria-autocomplete="list"
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
      {open && query.trim() && results.length > 0 ? (
        <ul id={listboxId} role="listbox" className="settings-search-results">
          {results.map((item, index) => (
            <li key={item.id} role="presentation">
              <button
                type="button"
                role="option"
                aria-selected={index === activeIndex}
                className={index === activeIndex ? "settings-search-result-active" : ""}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => selectItem(item)}
              >
                <strong>{item.title}</strong>
                <span>
                  {sectionLabel(item.section)} · {item.description}
                </span>
              </button>
            </li>
          ))}
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
