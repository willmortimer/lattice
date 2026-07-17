import type { PageMode } from "./pageDraft";

const MODE_LABELS: Record<PageMode, string> = {
  edit: "Edit",
  preview: "Preview",
  source: "Source",
};

export interface PageModeChromeProps {
  mode: PageMode;
  sourceParseError: string | null;
  onModeChange: (mode: PageMode) => void;
}

export function PageModeChrome({ mode, sourceParseError, onModeChange }: PageModeChromeProps) {
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
      {sourceParseError && mode === "source" && (
        <p className="page-mode-parse-error" role="status">
          Source could not be parsed into the page editor. Fix the markdown or keep editing here.
        </p>
      )}
    </div>
  );
}
