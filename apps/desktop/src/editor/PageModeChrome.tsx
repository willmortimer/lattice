import type { PageWidth } from "../lib/pageWidth";
import type { PageMode } from "./pageDraft";
import type { PagePersistMode } from "./collab/collabSession";

const MODE_LABELS: Record<PageMode, string> = {
  edit: "Edit",
  preview: "Preview",
  source: "Source",
};

const WIDTH_LABELS: Record<PageWidth, string> = {
  standard: "Standard",
  wide: "Wide",
  full: "Full",
};

const PERSIST_LABELS: Record<PagePersistMode, string> = {
  plain: "Plain file",
  collaborative: "Collaborative",
};

export interface PageModeChromeProps {
  mode: PageMode;
  sourceParseError: string | null;
  onModeChange: (mode: PageMode) => void;
  pageWidth: PageWidth;
  onPageWidthChange: (width: PageWidth) => void;
  persistMode?: PagePersistMode;
  onPersistModeChange?: (mode: PagePersistMode) => void;
  collaborativeAvailable?: boolean;
}

export function PageModeChrome({
  mode,
  sourceParseError,
  onModeChange,
  pageWidth,
  onPageWidthChange,
  persistMode = "plain",
  onPersistModeChange,
  collaborativeAvailable = false,
}: PageModeChromeProps) {
  return (
    <div className="page-mode-chrome">
      <div className="page-mode-tabs" role="tablist" aria-label="Page view mode">
        {(Object.keys(MODE_LABELS) as PageMode[]).map((candidate) => (
          <button
            key={candidate}
            type="button"
            role="tab"
            aria-selected={mode === candidate}
            className={mode === candidate ? "page-mode-tab page-mode-tab-active" : "page-mode-tab"}
            onClick={() => onModeChange(candidate)}
          >
            {MODE_LABELS[candidate]}
          </button>
        ))}
      </div>
      {collaborativeAvailable && onPersistModeChange ? (
        <div className="page-width-tabs" role="radiogroup" aria-label="Page persistence mode">
          {(Object.keys(PERSIST_LABELS) as PagePersistMode[]).map((candidate) => (
            <button
              key={candidate}
              type="button"
              role="radio"
              aria-checked={persistMode === candidate}
              className={
                persistMode === candidate ? "page-mode-tab page-mode-tab-active" : "page-mode-tab"
              }
              onClick={() => onPersistModeChange(candidate)}
            >
              {PERSIST_LABELS[candidate]}
            </button>
          ))}
        </div>
      ) : null}
      <div className="page-width-tabs" role="radiogroup" aria-label="Page width">
        {(Object.keys(WIDTH_LABELS) as PageWidth[]).map((candidate) => (
          <button
            key={candidate}
            type="button"
            role="radio"
            aria-checked={pageWidth === candidate}
            className={
              pageWidth === candidate ? "page-mode-tab page-mode-tab-active" : "page-mode-tab"
            }
            onClick={() => onPageWidthChange(candidate)}
          >
            {WIDTH_LABELS[candidate]}
          </button>
        ))}
      </div>
      {sourceParseError && mode === "source" && (
        <p className="page-mode-parse-error" role="status">
          Source could not be parsed into the page editor. Fix the markdown or keep editing here.
        </p>
      )}
    </div>
  );
}
