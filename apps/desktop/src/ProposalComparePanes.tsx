import { Group, Panel, Separator } from "react-resizable-panels";

import type { ProposalCompareSection } from "./lib/proposals";

const CURRENT_PANEL_ID = "current";
const PROPOSED_PANEL_ID = "proposed";

export interface ProposalComparePanesProps {
  sections: readonly ProposalCompareSection[];
  loading?: boolean;
  emptyMessage?: string;
}

function CompareColumn({
  title,
  side,
  sections,
}: {
  title: string;
  side: "current" | "proposed";
  sections: readonly ProposalCompareSection[];
}) {
  return (
    <div className="proposal-compare-column" data-side={side}>
      <header className="proposal-compare-column-head">
        <h3>{title}</h3>
      </header>
      <div className="proposal-compare-column-body">
        {sections.map((section, index) => {
          const text = side === "current" ? section.current : section.proposed;
          return (
            <article
              key={`${section.path ?? section.label}:${index}`}
              className="proposal-compare-section"
            >
              <header className="proposal-compare-section-head">
                <strong>{section.label}</strong>
                {section.path ? <code>{section.path}</code> : null}
              </header>
              {text ? (
                <pre
                  className={`proposal-command-excerpt proposal-compare-excerpt proposal-diff-${
                    side === "current" ? "before" : "after"
                  }`}
                >
                  {text}
                </pre>
              ) : (
                <p className="proposal-compare-empty">
                  {side === "current" ? "No current content" : "No proposed content"}
                </p>
              )}
            </article>
          );
        })}
      </div>
    </div>
  );
}

export function ProposalComparePanes({
  sections,
  loading = false,
  emptyMessage = "Select commands to preview Current and Proposed.",
}: ProposalComparePanesProps) {
  if (loading) {
    return (
      <div className="proposal-compare-panes proposal-compare-panes-status" role="status">
        Loading preview…
      </div>
    );
  }

  if (sections.length === 0) {
    return (
      <div className="proposal-compare-panes proposal-compare-panes-status">{emptyMessage}</div>
    );
  }

  return (
    <Group
      id="proposal-compare"
      className="proposal-compare-panes"
      orientation="horizontal"
      defaultLayout={{
        [CURRENT_PANEL_ID]: 50,
        [PROPOSED_PANEL_ID]: 50,
      }}
    >
      <Panel id={CURRENT_PANEL_ID} className="proposal-compare-panel" minSize="20%" defaultSize="50%">
        <CompareColumn title="Current" side="current" sections={sections} />
      </Panel>
      <Separator className="proposal-compare-resize-handle" />
      <Panel
        id={PROPOSED_PANEL_ID}
        className="proposal-compare-panel"
        minSize="20%"
        defaultSize="50%"
      >
        <CompareColumn title="Proposed" side="proposed" sections={sections} />
      </Panel>
    </Group>
  );
}
