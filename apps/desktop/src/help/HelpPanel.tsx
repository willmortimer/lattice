import { Button, IconButton } from "@lattice/ui";
import { X } from "@phosphor-icons/react";
import { useCallback, useEffect, useMemo, useState, type MouseEvent } from "react";

import { getGuidanceAnchor } from "../guidance";
import { openUrl } from "@tauri-apps/plugin-opener";
import { hasTauri } from "../lib/ipc";
import { inBrowser } from "../demo";

import {
  buildHelpCorpus,
  filterHelpPages,
  findHelpPageByStem,
  type HelpPage,
} from "./helpCorpus";
import { parseHelpDeepLinkUrl } from "./helpDeepLink";
import { HELP_NAVIGATION, HELP_RAW_BY_FILE } from "./helpManifest";
import { renderHelpMarkdownHtml } from "./helpMarkdown";
import "./help.css";

const CORPUS = buildHelpCorpus(HELP_NAVIGATION, HELP_RAW_BY_FILE);

export interface HelpPanelProps {
  deepLinkStem?: string | null;
  onDeepLinkConsumed?: () => void;
  onClose: () => void;
}

export function HelpPanel({ deepLinkStem, onDeepLinkConsumed, onClose }: HelpPanelProps) {
  const [query, setQuery] = useState("");
  const [selectedStem, setSelectedStem] = useState<string>(
    CORPUS.pages[0]?.stem ?? "welcome",
  );

  const filteredPages = useMemo(
    () => filterHelpPages(CORPUS.pages, query),
    [query],
  );

  const selectedPage = useMemo(
    () => findHelpPageByStem(CORPUS.pages, selectedStem) ?? CORPUS.pages[0] ?? null,
    [selectedStem],
  );

  useEffect(() => {
    if (!deepLinkStem) return;
    const page = findHelpPageByStem(CORPUS.pages, deepLinkStem);
    if (page) {
      setSelectedStem(page.stem);
      setQuery("");
    }
    onDeepLinkConsumed?.();
  }, [deepLinkStem, onDeepLinkConsumed]);

  const contentHtml = useMemo(
    () => (selectedPage ? renderHelpMarkdownHtml(selectedPage.body) : ""),
    [selectedPage],
  );

  const handleShowMe = useCallback(async () => {
    if (!selectedPage?.anchor) return;
    const anchor = getGuidanceAnchor(selectedPage.anchor);
    if (!anchor) return;
    await anchor.reveal();
  }, [selectedPage]);

  const handleContentClick = useCallback(
    (event: MouseEvent<HTMLDivElement>) => {
      const target = event.target;
      if (!(target instanceof Element)) return;
      const link = target.closest("a");
      if (!link) return;
      const href = link.getAttribute("href")?.trim() ?? "";
      if (!href) return;

      const helpStem = parseHelpDeepLinkUrl(href);
      if (helpStem) {
        event.preventDefault();
        const page = findHelpPageByStem(CORPUS.pages, helpStem);
        if (page) {
          setSelectedStem(page.stem);
          setQuery("");
        }
        return;
      }

      if (href.startsWith("http://") || href.startsWith("https://")) {
        event.preventDefault();
        if (hasTauri && !inBrowser) {
          void openUrl(href).catch(() => window.open(href, "_blank", "noopener,noreferrer"));
        } else {
          window.open(href, "_blank", "noopener,noreferrer");
        }
      }
    },
    [],
  );

  const navSections = useMemo(() => {
    if (!query.trim()) return CORPUS.navigation;
    const visibleFiles = new Set(filteredPages.map((page) => page.file));
    return CORPUS.navigation
      .map((section) => ({
        ...section,
        items: section.items.filter((item) => visibleFiles.has(item.file)),
      }))
      .filter((section) => section.items.length > 0);
  }, [filteredPages, query]);

  const showPageInNav = (page: HelpPage): boolean =>
    filteredPages.some((entry) => entry.stem === page.stem);

  return (
    <aside className="help-panel" aria-label="Help">
      <header className="help-head">
        <div>
          <div className="help-eyebrow">Lattice</div>
          <strong>Help</strong>
        </div>
        <IconButton label="Close Help" onClick={onClose}>
          <X size={14} />
        </IconButton>
      </header>
      <div className="help-search">
        <input
          type="search"
          value={query}
          placeholder="Search help…"
          aria-label="Search help"
          onChange={(event) => setQuery(event.target.value)}
        />
      </div>
      <div className="help-body">
        <nav className="help-nav" aria-label="Help topics">
          {navSections.map((section) => (
            <div className="help-nav-section" key={section.label}>
              <span className="help-nav-section-label">{section.label}</span>
              {section.items.map((item) => {
                const page = findHelpPageByStem(CORPUS.pages, stemFromItem(item.file));
                if (!page || !showPageInNav(page)) return null;
                return (
                  <button
                    type="button"
                    key={item.file}
                    className={
                      selectedPage?.stem === page.stem ? "help-nav-active" : undefined
                    }
                    onClick={() => setSelectedStem(page.stem)}
                  >
                    {item.label}
                  </button>
                );
              })}
            </div>
          ))}
        </nav>
        <div className="help-content">
          {!selectedPage || !showPageInNav(selectedPage) ? (
            <p className="help-empty">No help topics match your search.</p>
          ) : (
            <>
              {selectedPage.anchor && getGuidanceAnchor(selectedPage.anchor) && (
                <div className="help-show-me">
                  <Button variant="secondary" size="sm" onClick={() => void handleShowMe()}>
                    Show me
                  </Button>
                </div>
              )}
              <div
                className="markdown-body"
                onClick={handleContentClick}
                dangerouslySetInnerHTML={{ __html: contentHtml }}
              />
            </>
          )}
        </div>
      </div>
    </aside>
  );
}

function stemFromItem(file: string): string {
  return file.replace(/\.md$/i, "");
}
