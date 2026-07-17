import { useMemo, useState, type UIEvent } from "react";
import type { StructuredNode } from "./structuredParserCore";
import { defaultExpandedIds, flattenVisibleTree } from "./structuredTreeModel";

const ROW_HEIGHT = 26;
const VIEWPORT_HEIGHT = 420;
const OVERSCAN = 8;

export interface StructuredTreeProps {
  root: StructuredNode;
}

export function StructuredTree({ root }: StructuredTreeProps) {
  const [expanded, setExpanded] = useState(() => defaultExpandedIds(root));
  const [scrollTop, setScrollTop] = useState(0);
  const rows = useMemo(() => flattenVisibleTree(root, expanded), [expanded, root]);
  const first = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
  const last = Math.min(rows.length, Math.ceil((scrollTop + VIEWPORT_HEIGHT) / ROW_HEIGHT) + OVERSCAN);

  const toggle = (id: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleScroll = (event: UIEvent<HTMLDivElement>) => setScrollTop(event.currentTarget.scrollTop);

  return (
    <div className="lattice-structured-tree" role="tree" aria-label="Structured value" onScroll={handleScroll}>
      <div style={{ height: rows.length * ROW_HEIGHT, position: "relative" }}>
        {rows.slice(first, last).map((row, index) => {
          const position = first + index;
          return (
            <div
              className="lattice-structured-tree-row"
              key={row.id}
              role="treeitem"
              aria-level={row.level}
              aria-posinset={position + 1}
              aria-setsize={rows.length}
              aria-expanded={row.expandable ? row.expanded : undefined}
              tabIndex={0}
              style={{ top: position * ROW_HEIGHT, height: ROW_HEIGHT, paddingInlineStart: `${(row.level - 1) * 18 + 10}px` }}
              onClick={() => row.expandable && toggle(row.id)}
              onKeyDown={(event) => {
                if ((event.key === "Enter" || event.key === " ") && row.expandable) {
                  event.preventDefault();
                  toggle(row.id);
                }
              }}
            >
              <span aria-hidden="true" className="lattice-structured-tree-chevron">{row.expandable ? (row.expanded ? "▾" : "▸") : "·"}</span>
              <code>{row.label}</code>
            </div>
          );
        })}
      </div>
    </div>
  );
}
