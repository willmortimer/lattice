import { Button } from "@lattice/ui";
import { FilePlus, Files, Info, MagnifyingGlass, Sparkle } from "@phosphor-icons/react";

export function HomeDashboard({
  title,
  resourceCount,
  onNewPage,
  onQuickCapture,
  onFiles,
  onSearch,
  onInspect,
}: {
  title: string;
  resourceCount: number;
  onNewPage: () => void;
  onQuickCapture: () => void;
  onFiles: () => void;
  onSearch: () => void;
  onInspect: () => void;
}) {
  return (
    <div className="home-dashboard">
      <div className="home-welcome">
        <p className="home-eyebrow">Workspace home</p>
        <h1>{title}</h1>
        <p>{resourceCount} resources in a real directory.</p>
        <div>
          <Button variant="primary" onClick={onNewPage}>
            <FilePlus size={14} />
            New page
          </Button>
          <Button variant="secondary" onClick={onQuickCapture}>
            <Sparkle size={14} />
            Quick capture
          </Button>
        </div>
      </div>
      <div className="home-grid">
        <button type="button" onClick={onFiles}>
          <Files size={20} />
          <strong>Browse files</strong>
          <span>Navigate pages, canvases, tables, and native files.</span>
        </button>
        <button type="button" onClick={onSearch}>
          <MagnifyingGlass size={20} />
          <strong>Search workspace</strong>
          <span>Find indexed page content without loading the workspace into React.</span>
        </button>
        <button type="button" onClick={onInspect}>
          <Info size={20} />
          <strong>Inspect resources</strong>
          <span>Properties, links, history, source, schema, and diagnostics.</span>
        </button>
      </div>
    </div>
  );
}
